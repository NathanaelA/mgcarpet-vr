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
| `wizards.json` | no | JSON | Per-wizard configuration: AI stats, spell loadouts (MC2 since v1, MC1 since v2) |
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
  "bake_epoch": 1,
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
- `bake_epoch` (integer, `mgc_formats::BAKE_EPOCH`): the bake CONTENT
  epoch — bumped when the importer's output changes under an unchanged
  schema (a decode fix, corrected tables, a new member), so consumers
  can tell a baked tree is stale. Artifacts baked before the field
  existed deserialize as 0 (always stale). The game shell checks it at
  startup and rebakes automatically (see "Versioning and evolution").
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

### `wizards.json`

Exactly 8 blocks; slot 0 is the human player, 1–7 the AI wizards.
Both games bake one; per-game fields are omitted where not
applicable.

MC2 (from the level header block):

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

MC1/Hidden Worlds (format 2+; from the level record's 8 × 216-byte
per-player table at offset 37072 and the decoded 12-byte tail —
which `genparams.json`'s `footer` still preserves verbatim):

```json
{
  "wizards": [
    { "aggression": 128, "accuracy": 128, "tempo": 128,
      "castle_level": 0,
      "starting_spells": [1, 0, ...24 values...],
      "allowed_spells":  [1, 1, ...24 values...] }
  ],
  "player_count": 2,
  "tail_38800": 35
}
```

- `player_count`: active wizard slots (the engine services only slots
  below it; slot 0 = the human).
- `aggression`/`accuracy`/`tempo` (0–255): the AI personality —
  remc1 Type_160 `u16_522`/`u16_524`/`u16_526` (hate and war
  thresholds / aim cone / decision-and-burst cadence, turn agility,
  respawn delay).
- `castle_level`: 0 = no starting castle, N = the wizard spawns with
  a castle at level N−1 (AI slots; nonzero values on slot 0 appear
  on some levels — semantics for the human unverified).
- `starting_spells` (the record's var_230883) and `allowed_spells`
  (var_230983), 24 flags by MC1 spell ID: an AI wizard's level-start
  book grants spells flagged in BOTH; the human grant intersects
  `allowed_spells` with campaign-collected flags; `allowed_spells`
  also gates what an AI may learn mid-level.
- `tail_38800`: unexplained tail word, preserved verbatim.

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
3. Changing the CONTENT the importer emits — a decode fix, corrected
   tables, a new baked member — without touching the schema bumps
   `bake_epoch` (`mgc_formats::BAKE_EPOCH`, stamped into `meta.json`
   and every `bundle.json`). Schema versions answer "can this reader
   parse it"; the epoch answers "is this bake current". The game
   shell compares stamps at startup and reruns the full bake
   (`mgc_import::bake::bake_all` — the same path as `mgc-import
   bake`) when anything is missing or stale.
4. This file is updated in the same change as the code.

# Asset bundles

The second engine-facing format (Rust types + loader:
`mgc_formats::bundle`): everything translated from one game's asset
catalogs, as a directory of uniformly-named members. Where `.mgcl`
carries what is *level*-scoped, a bundle carries what is *world*-scoped
— palettes, color LUTs, terrain textures, sprites, terrain-feature
data; text will join additively. Sounds and music live in per-game
audio bundles (see "Audio bundles" below).

One schema, many *variants*: game and environment differences are
expressed as bundle instances, never as layout differences. Current
variants: `mc1-temperate`, `mc1-arctic` (MC1's two complete world
tilesets; Hidden Worlds levels use the arctic one) and MC2's four
environment graphics sets `mc2-day`, `mc2-night`, `mc2-night-fog`,
`mc2-cave` (per-level selector: the header's `map_type`, with night
levels whose `gfx_type` has bit 1 set using the fog set — remc2
Level.cpp:890). The engine resolves a variant id
(`baked/assets/<variant>/`); Bullfrog catalog names, RNC, sprite RLE,
and FLC animation encodings all die in the importer.

All integers little-endian; all pixel data 8-bit palette indices
(palette-as-LUT is the engine's authenticity baseline — RGBA is a
render-time resolve, and index 0 is the sprite-transparent index).

| member | contents |
|---|---|
| `bundle.json` | manifest: `format_version`, `bake_epoch` (same semantics as `meta.json`), `variant`, `game`, importer, source catalog files + raw-file sha256 |
| `palette.bin` | 256 x RGBA8, VGA 6-bit expanded (`v<<2\|v>>4`); index 0 has alpha 0 |
| `shade-lut.bin` | 64 rows x 256: `shade level x palette index -> final palette index` (the light/fog remap; the TABLES blob at +0x0000 in MC1, +0x4000 in MC2 — MC2 keeps a pixel-remap table at +0x0000) |
| `tile-colors.bin` | 256: terrain type -> flat map color index (TABLES blob +0x14000, both games) |
| `terrain-atlas.bin` + `.json` | terrain texture atlas, square cells; the json gives `{cell, width, cells}`; the terrain-type byte indexes cells row-major |
| `sprites.bin` + `sprites.json` | one 8bpp atlas of all world-sprite frames + its index (below); atlas width doubles from 1024 as needed to stay under the 8192 texture-dimension baseline |
| `search.bin` | 32x32 ring-order grid (terrain-feature digs) |
| `build.tab.bin`, `build.dat.bin` | building footprint RLE maps (terrain-feature building pass) |
| `ui-sprites.bin` + `.json` | 2D UI sprite library (HSPR: spell icons, HUD panel, mana-bar frames, level pips, map markers), same atlas + `SpriteIndex` schema as `sprites` with one frame per entry and `group == id`; entries 6..=29 are the 24 spell icons keyed by internal spell type, 83/84 the advertised-trigger map X-markers (MC1 only until MC2's UI track) |
| `book-palette.bin` | 256 x RGBA8 like `palette.bin`: the book/spellbook screen's own palette (MC1 `DATA/BOOK.PAL`) |
| `blend-lut.bin` | 64KB UI blend table (MC1 TABLES +0x4000..+0x14000, the slice between the shade LUT and map colors): 2D blits resolve `blend[src \| dest<<8]` — UI sprites (spell icons) only show their true colors composited through it (remc1 `strPal.byte_BB934_BB924`, sub_main.cpp:27444) |

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
(arctic). MC2 (all from the CD image): `PAL{D,N,F,C}-0.DAT`,
`TABLES{D,N,C}.DAT`, `BLOCK32.DAT` (day) / `BL32{N,F,C}0-0.DAT`,
`TMAPS{0,1,2}-0.DAT/.TAB` (digit = map_type ordinal; fog shares
night's tables and TMAPS). MC2 bundles carry no `search.bin` /
`build.*.bin` yet — its terrain-feature pass is a separate original
implementation, pending its own port. The versioning/evolution rules
above apply unchanged (`bundle.json` `format_version`).

## Audio bundles

Audio is per-GAME, not per-graphics-variant (MC1's sample/music bank
digits are level/screen selectors, not tileset pairs; duplicating the
redbook rip into four MC2 environment bundles would be absurd), so it
bakes as its own bundle instances: `mc1-audio`, `mc2-audio` — same
manifest schema, audio members only. Loader: `mgc_formats::bundle::
AudioBundle`.

| member | contents |
|---|---|
| `bundle.json` | same manifest schema as graphics bundles |
| `sounds.bin` | one deduplicated blob of raw PCM (unsigned 8-bit mono — the original sample data byte-for-byte) |
| `sounds.json` | `SoundIndex`: `sample_rate` (22050 — the best tier shipped by both retail games), `encoding` (`"pcm8"`), `banks[]` of `{bank, entries[]}`; each entry `{id, name, offset, len}` — `id` is the ENGINE sound id (the original bank-table index; the mixer's 47 request slots index bank 0 directly) |
| `music.json` | `MusicIndex`: `tracks[]` of `{bank, name, file, danger_file?, source}` |
| `music/*.flac` | one FLAC stream per track; MC1 in-game songs split into a base AMBIENT mix (`file`) plus a sample-aligned DANGER stem (`danger_file`, `*-danger.flac`) — the original keeps its combat layers on MIDI channels 3/4/5 at CC7 0 and fades them in/out with runtime CC7 ramps (remc1 sub_20BD0/sub_20D00); the runtime overlays the stem with the same ramp. Songs without a muted danger layer (menu/intro) and redbook tracks have no stem |

Sources: MC1 `DATA/SNDS<bank>-<q>.DAT/.TAB` (bank 0 = the 47-sound
gameplay bank, 1..13 auxiliary sets; `q` = the original's free-RAM
quality tier, always baked from `-1` = 22050 Hz) and
`DATA/MUSIC{0,1}-0.DAT/.TAB` — HMP songs rendered through OPL3
(nuked-opl3) with the game's own `INST.BNK`/`DRUM.BNK` AdLib patches
at import, 44100 Hz mono FLAC (`0-cgame1` … `1-cintro6`). MC2
`SOUND/SOUND.DAT` (10 banks, best shipped tier = 8-bit 22050; the
per-sample WAV containers are stripped to keep `sounds.bin` raw PCM)
and the 27 redbook audio tracks ripped losslessly from the CD image
(`game.gog` + `game.ins` cue) as 44100 Hz stereo FLAC
(`track-02` … `track-28`) — CD audio was retail MC2's primary music
path; its AIL XMI arrangement is a future faithful-alternate.
