//! Bundle baking: original per-game catalogs in, unified asset bundles
//! out (`baked/assets/<variant>/`, schema in `mgc_formats::bundle` and
//! docs/FORMAT.md).
//!
//! MC1 ships two complete world tilesets — 0 = temperate, 1 = arctic
//! (snow; used by the Hidden Worlds bundle) — each with its own palette,
//! color tables, terrain atlas, sprites, and building footprints. They
//! bake as the `mc1-temperate` and `mc1-arctic` variants. MC2's
//! day/night/cave variants use the same schema once its CD catalogs are
//! available.

use std::path::Path;

use sha2::{Digest, Sha256};

use mgc_formats::bundle::{
    BUNDLE_VERSION, BundleManifest, BundleSource, TerrainAtlasInfo,
};
use mgc_formats::{Game, Importer};

use crate::bake::BakeError;
use crate::sprites;
use crate::tmaps::TmapsArchive;

/// Width of the baked sprite atlas; retail sprites max out well below.
const SPRITE_ATLAS_WIDTH: u32 = 1024;

const SHADE_LUT_LEN: usize = 0x4000; // 64 shade levels x 256 colors
const TILE_COLORS_OFFSET: usize = 0x14000;
const TABLES_LEN: usize = 0x14600; // the engine's full color-table blob
const ATLAS_CELL: u32 = 32;
const ATLAS_WIDTH: u32 = 256;
const ATLAS_CELLS: u32 = 152;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bake MC1's two world tilesets into `out_dir/assets/{mc1-temperate,
/// mc1-arctic}`. Returns `(manifest path, sha256)` pairs for every
/// written member.
///
/// Original catalogs consumed per set N (0 = temperate, 1 = arctic):
/// - `PALN-0.DAT` (RNC, 6-bit VGA palette) → `palette.bin`, expanded to
///   RGBA8 (`v<<2 | v>>4`); index 0 gets alpha 0 (the engine's sprite
///   transparent index), everything else alpha 255.
/// - `TABLES.DAT` / `DTABLES.DAT` (set 1, stored raw) → `shade-lut.bin`
///   (first 0x4000 bytes) and `tile-colors.bin` (+0x14000), exactly the
///   remc2-documented layout.
/// - `BLKN-1.DAT` (RNC) → `terrain-atlas.bin` + `terrain-atlas.json`
///   (32px cells, the terrain-type byte is the cell index).
/// - `TMAPSN-0.DAT/.TAB` → `sprites.bin` + `sprites.json` (world
///   billboards; FLC animations pre-decoded, see `crate::sprites`).
/// - `BUILDN-0.TAB/.DAT` (RNC) → `build.tab.bin`/`build.dat.bin`, and
///   `SEARCH.DAT` (RNC, tileset-independent) → `search.bin`: the
///   terrain-feature pass data (`mgc_sim::features`).
pub fn bake_mc1_bundles(
    data_dir: &Path,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    let mut outputs = Vec::new();
    for (set, variant) in [(0u8, "mc1-temperate"), (1u8, "mc1-arctic")] {
        let dir = out_dir.join("assets").join(variant);
        std::fs::create_dir_all(&dir).map_err(|e| BakeError::Io(dir.clone(), e))?;
        let baked = bake_variant(data_dir, &dir, set, variant)?;
        outputs.extend(
            baked
                .into_iter()
                .map(|(name, sha)| (format!("assets/{variant}/{name}"), sha)),
        );
    }
    // The pre-bundle asset layout; remove so stale files cannot shadow
    // the bundles.
    let legacy = out_dir.join("mc1/assets");
    if legacy.is_dir() {
        std::fs::remove_dir_all(&legacy).map_err(|e| BakeError::Io(legacy.clone(), e))?;
    }
    Ok(outputs)
}

fn bake_variant(
    data_dir: &Path,
    dir: &Path,
    set: u8,
    variant: &str,
) -> Result<Vec<(String, String)>, BakeError> {
    let mut outputs = Vec::new();
    let mut sources = Vec::new();

    let mut emit = |name: &str, bytes: &[u8]| -> Result<(), BakeError> {
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| BakeError::Io(path, e))?;
        outputs.push((name.to_string(), hex(&Sha256::digest(bytes))));
        Ok(())
    };
    // Read + record provenance; decompress whole-file RNC when present
    // (DTABLES.DAT and the TMAPS TABs ship raw).
    let source = |file: &str, sources: &mut Vec<BundleSource>| -> Result<Vec<u8>, BakeError> {
        let path = data_dir.join(file);
        let raw = std::fs::read(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        sources.push(BundleSource {
            file: file.to_string(),
            sha256: hex(&Sha256::digest(&raw)),
        });
        if crate::rnc::is_rnc(&raw) {
            crate::rnc::decompress(&raw).map_err(|e| BakeError::Level(path, 0, e.to_string()))
        } else {
            Ok(raw)
        }
    };
    let expect = |file: &str, data: &[u8], len: usize| -> Result<(), BakeError> {
        if data.len() != len {
            return Err(BakeError::Level(
                data_dir.join(file),
                0,
                format!("{} bytes, expected {len}", data.len()),
            ));
        }
        Ok(())
    };

    // Palette: 6-bit VGA -> RGBA8, index 0 transparent.
    let pal_file = format!("PAL{set}-0.DAT");
    let vga = source(&pal_file, &mut sources)?;
    expect(&pal_file, &vga, 768)?;
    let mut rgba = Vec::with_capacity(1024);
    for (i, c) in vga.chunks_exact(3).enumerate() {
        for &v in c {
            rgba.push((v << 2) | (v >> 4));
        }
        rgba.push(if i == 0 { 0 } else { 255 });
    }
    emit("palette.bin", &rgba)?;

    // Color tables.
    let tables_file = if set == 0 { "TABLES.DAT" } else { "DTABLES.DAT" };
    let tables = source(tables_file, &mut sources)?;
    expect(tables_file, &tables, TABLES_LEN)?;
    emit("shade-lut.bin", &tables[..SHADE_LUT_LEN])?;
    emit(
        "tile-colors.bin",
        &tables[TILE_COLORS_OFFSET..TILE_COLORS_OFFSET + 256],
    )?;

    // Terrain atlas.
    let blk_file = format!("BLK{set}-1.DAT");
    let atlas = source(&blk_file, &mut sources)?;
    expect(
        &blk_file,
        &atlas,
        (ATLAS_WIDTH * ATLAS_CELL * ATLAS_CELLS.div_ceil(ATLAS_WIDTH / ATLAS_CELL)) as usize,
    )?;
    emit("terrain-atlas.bin", &atlas)?;
    emit(
        "terrain-atlas.json",
        &serde_json::to_vec_pretty(&TerrainAtlasInfo {
            cell: ATLAS_CELL,
            width: ATLAS_WIDTH,
            cells: ATLAS_CELLS,
        })
        .expect("terrain atlas info serializes"),
    )?;

    // World sprites.
    let tmaps_dat_file = format!("TMAPS{set}-0.DAT");
    let tmaps_tab_file = format!("TMAPS{set}-0.TAB");
    let tmaps_dat = source(&tmaps_dat_file, &mut sources)?;
    let tmaps_tab = source(&tmaps_tab_file, &mut sources)?;
    let archive = TmapsArchive::open(&tmaps_dat, &tmaps_tab)
        .map_err(|e| BakeError::Level(data_dir.join(&tmaps_dat_file), 0, e.to_string()))?;
    let (decoded, warnings) = sprites::decode_tmaps(&archive)
        .map_err(|e| BakeError::Level(data_dir.join(&tmaps_dat_file), 0, e.to_string()))?;
    for w in warnings {
        eprintln!("note: {variant}: {w}");
    }
    let packed = sprites::pack(&decoded, SPRITE_ATLAS_WIDTH);
    emit("sprites.bin", &packed.atlas)?;
    emit(
        "sprites.json",
        &serde_json::to_vec_pretty(&packed.index).expect("sprite index serializes"),
    )?;

    // Terrain-feature data.
    let tab_file = format!("BUILD{set}-0.TAB");
    let tab = source(&tab_file, &mut sources)?;
    if tab.len() % 6 != 0 {
        return Err(BakeError::Level(
            data_dir.join(&tab_file),
            0,
            format!("{} bytes is not 6-byte entries", tab.len()),
        ));
    }
    emit("build.tab.bin", &tab)?;
    let dat_file = format!("BUILD{set}-0.DAT");
    let build_dat = source(&dat_file, &mut sources)?;
    emit("build.dat.bin", &build_dat)?;
    let search = source("SEARCH.DAT", &mut sources)?;
    expect("SEARCH.DAT", &search, 1024)?;
    emit("search.bin", &search)?;

    let manifest = BundleManifest {
        format_version: BUNDLE_VERSION,
        variant: variant.to_string(),
        game: Game::MagicCarpet1,
        importer: Importer {
            name: "mgc-import".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        sources,
    };
    emit(
        "bundle.json",
        &serde_json::to_vec_pretty(&manifest).expect("manifest serializes"),
    )?;
    Ok(outputs)
}
