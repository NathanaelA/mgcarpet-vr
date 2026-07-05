//! Asset bundles: the unified, engine-facing asset format.
//!
//! A bundle is a directory of assets translated from one original
//! game's catalogs — palettes, color LUTs, terrain textures, sprites,
//! terrain-feature data — under uniform member names shared by every
//! game. Game differences are expressed as bundle *variants* (MC1
//! temperate/arctic world tilesets, MC2 day/night/cave environments),
//! never as schema differences: the engine resolves a variant id and
//! reads one layout.
//!
//! Like `.mgcl`, the bundle is the only asset contract between
//! `mgc-import` and the engine crates; Bullfrog catalog names, RNC,
//! RLE and FLC encodings all die in the importer.
//!
//! **The normative specification lives in `docs/FORMAT.md`** ("Asset
//! bundles"); keep this module and that document in lockstep.
//!
//! Members (all little-endian, pixel data 8-bit palette indices):
//! - `bundle.json` — [`BundleManifest`]
//! - `palette.bin` — 256 x RGBA8 (index 0 is the transparent index for
//!   sprite data; alpha is 0 there, 255 elsewhere)
//! - `shade-lut.bin` — 64 rows x 256: shade level x palette index ->
//!   final palette index (the engine's light/fog remap)
//! - `tile-colors.bin` — 256: terrain type -> flat map color index
//! - `terrain-atlas.bin` + `terrain-atlas.json` — square-cell terrain
//!   texture atlas ([`TerrainAtlasInfo`])
//! - `sprites.bin` + `sprites.json` — one 8bpp sprite atlas + its
//!   index ([`SpriteIndex`]; billboard frames, animations pre-decoded)
//! - `search.bin`, `build.tab.bin`, `build.dat.bin` — terrain-feature
//!   pass data (ring search order, building footprints)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Game, Importer};

/// Current bundle format version.
pub const BUNDLE_VERSION: u32 = 1;

/// `bundle.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub format_version: u32,
    /// Variant id; by convention also the bundle directory name
    /// (e.g. `mc1-temperate`, `mc1-arctic`, `mc2-day`).
    pub variant: String,
    /// Game whose catalogs the bundle was translated from.
    pub game: Game,
    pub importer: Importer,
    /// Original catalog files consumed, with their raw-file hashes.
    pub sources: Vec<BundleSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleSource {
    pub file: String,
    pub sha256: String,
}

/// `terrain-atlas.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainAtlasInfo {
    /// Edge length of one square cell in pixels.
    pub cell: u32,
    /// Atlas width in pixels (cells per row = `width / cell`).
    pub width: u32,
    /// Number of cells; the terrain-type byte indexes them row-major.
    pub cells: u32,
}

/// `sprites.json`: the index into the `sprites.bin` atlas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteIndex {
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Indexed by the original engine's sprite id (dense; broken
    /// source entries are kept as frame-less placeholders so ids
    /// stay aligned).
    pub sprites: Vec<SpriteEntry>,
}

/// One logical sprite: all frames share one size; frame 0 is the base
/// image, further frames are the pre-decoded animation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteEntry {
    pub id: u32,
    /// Id of the first sprite of the family this one belongs to (a
    /// creature's rotation views, an animation set); `== id` for a
    /// family head or a standalone sprite.
    pub group: u32,
    pub width: u16,
    pub height: u16,
    /// Original archive flags, preserved for provenance/debugging.
    pub flags: u16,
    /// Atlas position of each frame; empty when the source entry is
    /// broken (known retail data corruption) — renderers skip these.
    pub frames: Vec<FramePos>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramePos {
    pub x: u32,
    pub y: u32,
}

/// A fully-loaded bundle.
#[derive(Debug, Clone)]
pub struct Bundle {
    pub manifest: BundleManifest,
    /// 256 RGBA entries.
    pub palette: [[u8; 4]; 256],
    /// 64 x 256 (shade level x palette index).
    pub shade_lut: Vec<u8>,
    pub tile_colors: [u8; 256],
    pub terrain_atlas: Option<(TerrainAtlasInfo, Vec<u8>)>,
    pub sprites: Option<(SpriteIndex, Vec<u8>)>,
    pub search: Option<Vec<u8>>,
    pub build_tab: Option<Vec<u8>>,
    pub build_dat: Option<Vec<u8>>,
}

/// Rows in `shade-lut.bin`.
pub const SHADE_LEVELS: usize = 64;

#[derive(Debug)]
pub enum BundleError {
    Io(PathBuf, std::io::Error),
    Json(PathBuf, serde_json::Error),
    /// A member has the wrong size for its declared shape.
    Malformed(PathBuf, String),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Json(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Malformed(p, m) => write!(f, "{}: {m}", p.display()),
        }
    }
}

impl std::error::Error for BundleError {}

impl Bundle {
    /// Load a bundle directory. Optional members (atlas, sprites,
    /// feature data) load as `None` when absent; the manifest, palette
    /// and color LUTs are required.
    pub fn load(dir: &Path) -> Result<Self, BundleError> {
        let read = |name: &str| -> Result<Vec<u8>, BundleError> {
            let p = dir.join(name);
            std::fs::read(&p).map_err(|e| BundleError::Io(p, e))
        };
        let read_opt = |name: &str| -> Option<Vec<u8>> { std::fs::read(dir.join(name)).ok() };
        let expect = |name: &str, data: &[u8], len: usize| -> Result<(), BundleError> {
            if data.len() != len {
                return Err(BundleError::Malformed(
                    dir.join(name),
                    format!("{} bytes, expected {len}", data.len()),
                ));
            }
            Ok(())
        };

        let manifest_path = dir.join("bundle.json");
        let manifest: BundleManifest = serde_json::from_slice(&read("bundle.json")?)
            .map_err(|e| BundleError::Json(manifest_path, e))?;

        let palette_bytes = read("palette.bin")?;
        expect("palette.bin", &palette_bytes, 256 * 4)?;
        let mut palette = [[0u8; 4]; 256];
        for (i, rgba) in palette.iter_mut().enumerate() {
            rgba.copy_from_slice(&palette_bytes[i * 4..i * 4 + 4]);
        }

        let shade_lut = read("shade-lut.bin")?;
        expect("shade-lut.bin", &shade_lut, SHADE_LEVELS * 256)?;

        let tile_colors_bytes = read("tile-colors.bin")?;
        expect("tile-colors.bin", &tile_colors_bytes, 256)?;
        let mut tile_colors = [0u8; 256];
        tile_colors.copy_from_slice(&tile_colors_bytes);

        let terrain_atlas = match read_opt("terrain-atlas.bin") {
            Some(data) => {
                let info_path = dir.join("terrain-atlas.json");
                let info: TerrainAtlasInfo = serde_json::from_slice(&read("terrain-atlas.json")?)
                    .map_err(|e| BundleError::Json(info_path, e))?;
                let cell_rows = info.cells.div_ceil(info.width / info.cell);
                expect(
                    "terrain-atlas.bin",
                    &data,
                    (info.width * info.cell * cell_rows) as usize,
                )?;
                Some((info, data))
            }
            None => None,
        };

        let sprites = match read_opt("sprites.bin") {
            Some(data) => {
                let index_path = dir.join("sprites.json");
                let index: SpriteIndex = serde_json::from_slice(&read("sprites.json")?)
                    .map_err(|e| BundleError::Json(index_path, e))?;
                expect(
                    "sprites.bin",
                    &data,
                    index.atlas_width as usize * index.atlas_height as usize,
                )?;
                Some((index, data))
            }
            None => None,
        };

        Ok(Self {
            manifest,
            palette,
            shade_lut,
            tile_colors,
            terrain_atlas,
            sprites,
            search: read_opt("search.bin"),
            build_tab: read_opt("build.tab.bin"),
            build_dat: read_opt("build.dat.bin"),
        })
    }
}
