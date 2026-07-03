//! The baked package format: the only data contract between `mgc-import`
//! and the engine crates.
//!
//! The importer expands original game data (RNC/DAT/TAB, seeded terrain
//! generation, XMI music, ...) into packages of this format, once per
//! machine. The engine consumes packages exclusively — it knows nothing
//! about Bullfrog formats, seeds, or RNG sequencing.
//!
//! The concrete layout is intentionally undefined at this stage; it will
//! grow field by field as the importer learns to produce real data.

/// Bumped on every incompatible change to the baked layout. The engine
/// refuses packages with a mismatching version; the fix is always to
/// re-run the importer.
pub const PACKAGE_FORMAT_VERSION: u32 = 0;

/// Which original game an asset belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Game {
    /// Magic Carpet (1994). GOG's "Magic Carpet Plus" edition.
    MagicCarpet1,
    /// The Hidden Worlds expansion (1995), shipped inside Magic Carpet Plus.
    HiddenWorlds,
    /// Magic Carpet 2: The Netherworlds (1995).
    MagicCarpet2,
}

/// Placeholder for a fully-expanded, ready-to-load level.
///
/// Terrain arrives pre-generated (the importer runs the original
/// generation code); the engine never sees a seed.
#[derive(Debug)]
pub struct LevelPackage {
    pub game: Game,
    /// Index of the level within its campaign.
    pub level: u32,
}
