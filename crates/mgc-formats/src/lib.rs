//! The `.mgcl` level-package format: the only data contract between
//! `mgc-import` and the engine crates.
//!
//! The importer expands original game data into packages of this format,
//! once per machine. The engine consumes packages exclusively — it knows
//! nothing about Bullfrog formats, seeds, or RNG sequencing.
//!
//! **The normative specification lives in `docs/FORMAT.md`.** Keep this
//! module and that document in lockstep: format changes land in the same
//! commit as their documentation.

use serde::{Deserialize, Serialize};

pub mod bundle;
pub mod mgcl;

/// Current `.mgcl` format version (see docs/FORMAT.md "Versioning").
/// 2: MC1 packages gained `wizards.json` (per-player AI records +
/// decoded level tail); MC2's wizard fields regrouped as optionals.
pub const FORMAT_VERSION: u32 = 2;

/// Current bake CONTENT epoch (see docs/FORMAT.md "Versioning").
/// Orthogonal to the schema versions above: bump it whenever the
/// importer's OUTPUT changes under an unchanged schema (a decode fix,
/// a new baked member, corrected tables), so consumers know a baked
/// tree is stale and must be regenerated from game data. Artifacts
/// baked before the field existed deserialize as epoch 0 — always
/// stale.
/// 1: first stamped epoch (2026-07-09).
/// 2: mc2 bundles gain search.bin + bldgprm.bin (Phase 3).
/// 3: mc2 bundles gain build.tab.bin + build.dat.bin (BUILD0-0 —
///    the building footprint bank; Phase 3.5 building creator).
/// 4: mc2 bundles gain ui-sprites (HSPR{D,N,C}0-0) + blend-lut.bin
///    (the CTRL spell-selector pane track).
/// 5: mc2 shade-lut.bin re-carved from TABLES +0x0000 — the +0x4000
///    slice it used to carry is the sprite blend matrix, not the
///    shade LUT (docs/traces/mc2-transparency-drawlist.md).
/// 6: mc2 bundles gain spells.bin (SPELLS.DAT verbatim, 26x80 — the
///    par1-authored class-10 overrides + class-15 cast costs; the
///    retail CD table differs from the decompile's baked-in fallback).
/// 7: mc1-audio gains the General MIDI arrangement (`MUSIC<bank>-2`
///    rendered via fluidsynth, `music/*-gm[-danger].flac` + the
///    gm_file/gm_danger_file music.json fields) on hosts that can
///    render GM; FM-only hosts re-bake to the same pre-7 content.
/// 8: mc2 cave levels carry `terrain/ceiling.bin` (the second
///    heightmap, oracle plane +0x40000) and the oracle now seeds
///    `MapBasicHeight` from header byte 0x05 (pre-8 cave bakes
///    mirrored about the weak default 44, so their angle plane's
///    sealed-bit-3 mask was wrong too); `level.json` renames
///    unk05 → basic_height.
/// 9: the MC2 audio column — mc2-audio drops the interim redbook
///    `track-NN` music members for (a) the MUSIC.DAT GM bank-1 XMI
///    renders (`music/mc2-{night,day,cave,menu}.flac` + war-channel
///    danger stems) and (b) the voiceover clips (`speech/*.flac` +
///    `speech.json`, the CdTracks_DB080 slices); `wizards.json`
///    renames unknown_spells → starting_spell_levels and
///    `level.json` renames unk09 → number_of_players.
/// 10: every graphics bundle gains `font.bin`/`font.json` — the
///    messaging/notification bitmap font (`DATA/FONT1`, the small
///    ~4x7 font both games draw the top-of-screen toast with),
///    HSPR-format 1-bit glyph masks packed to one atlas (sprite id =
///    ASCII char + 1). Feeds the top-of-screen notification surface.
/// 11: mc2-audio gameplay music re-baked from MUSIC.DAT GM **bank 0**
///    (the "C2"/Magic Carpet 2 set — the default `musicChannel_E3814`),
///    correcting the bank-1 "C1"/MC1 tracks that were wrongly shipping
///    as gameplay music (docs/traces/mc2-music-law.md). Night/Day/Cave
///    now = C2GAME1/2/3; the sparse ~80bpm C2GAME3 restores the quiet
///    cave. `music.json` track `bank` field flips 1→0.
pub const BAKE_EPOCH: u32 = 11;

/// Which original game an asset belongs to. Serialized as the short
/// tags used in `meta.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Game {
    /// Magic Carpet (1994). GOG's "Magic Carpet Plus" edition.
    #[serde(rename = "mc1")]
    MagicCarpet1,
    /// The Hidden Worlds expansion (1995), shipped inside Magic Carpet Plus.
    #[serde(rename = "mc1hw")]
    HiddenWorlds,
    /// Magic Carpet 2: The Netherworlds (1995).
    #[serde(rename = "mc2")]
    MagicCarpet2,
}

/// `meta.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub format_version: u32,
    /// Bake content epoch (`BAKE_EPOCH` at bake time); pre-epoch
    /// artifacts read as 0.
    #[serde(default)]
    pub bake_epoch: u32,
    pub game: Game,
    /// Index of the level in its source archive.
    pub level: u32,
    /// Provenance; absent on community-authored levels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub importer: Importer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub archive: String,
    pub entry: u32,
    /// Hex SHA-256 of the raw (still-compressed) archive entry.
    pub entry_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Importer {
    pub name: String,
    pub version: String,
}

/// One record in `things.json`. Field slots are shared across games;
/// MC2 semantics for the shared slots (`swi_id` = stage tag,
/// `parent`/`child` = context parameters) are documented in
/// docs/FORMAT.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thing {
    /// Original entity-table slot; `parent`/`child` reference these.
    pub slot: u32,
    pub kind: ThingKind,
    pub class: u16,
    pub model: u16,
    pub x: u16,
    pub y: u16,
    pub dis_id: u16,
    pub swi_sz: u16,
    pub swi_id: u16,
    pub parent: u16,
    pub child: u16,
    /// MC2 only (third context parameter); absent on MC1 records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub par3: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThingKind {
    Entity,
    Marker,
}

/// `things.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Things {
    pub things: Vec<Thing>,
}

/// `genparams.json` — original GEN_MAP terrain parameters, kept for
/// provenance. Engines must not require this; authoritative terrain is
/// the expanded `terrain/*` members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenParams {
    /// MC1 only: unexplained pre-GEN_MAP header field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_header: Option<u32>,
    pub seed: u32,
    pub off: u32,
    pub raise: i32,
    pub gnarl: u32,
    pub river: u32,
    /// MC2 only: river length/count parameter absent from MC1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lriver: Option<u32>,
    pub sourc: u32,
    pub snlin: u32,
    pub snflt: u32,
    pub bhlin: u32,
    pub bhflt: u32,
    pub rkste: u32,
    /// MC1 only: trailing 12-byte level footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<[u16; 6]>,
}

/// `level.json` — MC2 level header (absent on MC1 packages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelHeader {
    pub level_id: u16,
    pub gfx_type: u8,
    pub map_type: MapType,
    /// `player_0x2FED9[8]` — authored starting-castle LEVEL per
    /// wizard color (0 = none, N = a castle at level N-1 built at
    /// the wizard's spawn; consumers EF:43777/43789,
    /// docs/traces/mc2-castle-data-tables.md §3). Mis-documented as
    /// "activation flags" before the castle-column trace.
    pub players: [i8; 8],
    /// Cave basic height (header byte 0x05 = `byte_0x2FED3`): the
    /// ceiling mirror pivot on cave levels (`MapBasicHeight_D41B7`,
    /// LevelInit.cpp:36; docs/traces/mc2-cave-terrain-foundation.md
    /// §4.1). Meaningless off-cave (retail keeps the default 44).
    /// Named `unk05` before the field was identified.
    #[serde(alias = "unk05")]
    pub basic_height: u8,
    /// `word_0x2FED5`, preserved verbatim (retail repurposes the
    /// field at runtime as objective scratch; no load consumer).
    pub unk07: i16,
    /// `word_0x2FED7` = NumberOfPlayers: colors 0..n-1 spawn wizard
    /// carpets (0 = the human in single player, 1..n-1 = AI rivals
    /// — docs/traces/mc2-rivals-spawn-mortality.md §1). Named
    /// `unk09` before the field was identified (renamed at EPOCH 9).
    #[serde(alias = "unk09")]
    pub number_of_players: i16,
}

/// Environment type; selects which asset set (day/night/cave variants
/// of sprites, sky, palette, tables, blocks) the level uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapType {
    Day,
    Night,
    Cave,
}

/// `wizards.json` — per-level wizard configuration. Both games carry
/// one: MC2's level header block, and (since format 2) MC1's level-
/// record tail — the 8 x 216-byte per-player records at offset 37072
/// plus the decoded 12-byte tail the spec used to call the "footer"
/// (which `GenParams::footer` still preserves verbatim).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wizards {
    /// Exactly 8 blocks: slot 0 = human player, 1-7 = AI wizards.
    pub wizards: Vec<WizardConfig>,
    /// MC1 only: active wizard count (level tail u16 @38802; the
    /// engine services only player slots below it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_count: Option<u16>,
    /// MC1 only: the unexplained map-screen coordinate word (level
    /// tail u16 @38800), preserved verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_38800: Option<u16>,
}

/// One wizard slot. `aggression` is shared; the other personality
/// fields are per-game (MC2: reflexes/perception/life; MC1: accuracy/
/// tempo — remc1 Type_160 u16_524/u16_526). `starting_spells` is
/// shared: the spells GRANTED at level start (MC2:
/// `StartingSpells_0x360E1x` grant flags by MC2 spell id, consumed by
/// InitialiseSpells_54A50 EF:38650; MC1: the var_230883 pre-grant
/// mask by MC1 spell id, granted iff `allowed_spells` also flags the
/// slot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WizardConfig {
    pub aggression: i16,
    /// MC2 only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflexes: Option<i16>,
    /// MC2 only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perception: Option<i16>,
    /// MC2 only: AI life scale, 16.8 (also scales maxLife,
    /// EF:43768-71; the human is always 256).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life: Option<i16>,
    /// Per-spell starting grant flags, indexed by the game's spell
    /// ID (MC2: 26 flags; MC1: 24 flags).
    pub starting_spells: Vec<u8>,
    /// MC2 only: per-spell STARTING XP LEVEL 0..2 (`byte_0x360FBx` —
    /// identified 2026-07-12, docs/traces/mc2-rivals-spawn-mortality
    /// .md §3: an AI's `SpellLevels[spell]` seeds from this, clamped
    /// ≤2). Named `unknown_spells` before identification (renamed at
    /// EPOCH 9).
    #[serde(
        default,
        alias = "unknown_spells",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub starting_spell_levels: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_spells: Vec<u8>,
    /// MC1 only: AI aim accuracy (u16_524 — commit aim cone,
    /// rebound-notice probability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<i16>,
    /// MC1 only: AI tempo (u16_526 — decision period, turn agility,
    /// burst pause, respawn delay).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<i16>,
    /// MC1 only: starting castle level (level tail @38804+slot; 0 =
    /// none, N = a castle at level N-1 spawns with the wizard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub castle_level: Option<u8>,
    /// MC1 only: the var_230983 availability mask by spell id — the
    /// human grant intersects it with campaign-collected flags; the
    /// AI's learn-eligibility list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_spells: Option<Vec<u8>>,
}

/// `stages.json` — MC2 mission script (absent on MC1 packages).
/// Interpretation of checkpoints/variables into objective opcodes is
/// documented in docs/FORMAT.md; the data is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stages {
    pub checkpoints: Vec<StageCheckpoint>,
    pub variables: Vec<StageVar>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageCheckpoint {
    pub index: i8,
    pub stage: i16,
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageVar {
    pub index: i8,
    pub stage: i8,
    pub x: u8,
    pub y: u8,
    pub data: u32,
}

/// Expanded terrain (`terrain/*.bin` members): the pristine output of
/// the original generation algorithm, before any entity-driven terrain
/// modification (walls, canyons, building flattening — those are applied
/// by the engine at load time from `things.json`).
///
/// Both grids are 256x256, one byte per tile, row-major, index
/// `y * 256 + x`, matching the original engine's in-memory layout.
#[derive(Clone, PartialEq, Eq)]
pub struct Terrain {
    /// `terrain/height.bin`.
    pub height: Vec<u8>,
    /// `terrain/type.bin` (per-tile terrain/texture type).
    pub tile_type: Vec<u8>,
    /// `terrain/shading.bin` (per-tile light level; indexes the shade
    /// dimension of the game's color-remap tables). Optional: packages
    /// baked before this member existed omit it.
    pub shading: Option<Vec<u8>>,
    /// `terrain/angle.bin` (per-tile texture-orientation/flags byte;
    /// bits 4-6 select one of 8 UV orientations for the tile's terrain
    /// texture). Optional: packages baked before this member existed
    /// omit it.
    pub angle: Option<Vec<u8>>,
    /// `terrain/ceiling.bin` — MC2 cave second heightmap
    /// (`x_BYTE_14B4E0`, the oracle block's +0x40000 plane): the cave
    /// ceiling, world height = 32 * value like the floor. Present only
    /// on cave levels (docs/traces/mc2-cave-terrain-foundation.md);
    /// day/night packages and pre-cave bakes omit it.
    pub ceiling: Option<Vec<u8>>,
}

impl std::fmt::Debug for Terrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Terrain {{ 256x256 }}")
    }
}

pub const TERRAIN_GRID_BYTES: usize = 256 * 256;

/// A fully-loaded level package (the members the current format version
/// defines; unknown members are preserved at the container level, see
/// [`mgcl`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelPackage {
    pub meta: Meta,
    pub things: Things,
    pub gen_params: Option<GenParams>,
    pub header: Option<LevelHeader>,
    pub wizards: Option<Wizards>,
    pub stages: Option<Stages>,
    pub terrain: Option<Terrain>,
}
