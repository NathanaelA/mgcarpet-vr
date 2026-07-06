//! Bundle baking: original per-game catalogs in, unified asset bundles
//! out (`baked/assets/<variant>/`, schema in `mgc_formats::bundle` and
//! docs/FORMAT.md).
//!
//! MC1 ships two complete world tilesets — 0 = temperate, 1 = arctic
//! (snow; used by the Hidden Worlds bundle) — each with its own palette,
//! color tables, terrain atlas, sprites, and building footprints. They
//! bake as the `mc1-temperate` and `mc1-arctic` variants. MC2 ships
//! four environment graphics sets from its CD catalogs — `mc2-day`,
//! `mc2-night`, `mc2-night-fog` (night levels with gfx_type bit 1),
//! `mc2-cave` — same schema, no build/search members yet (its
//! terrain-feature pass is a separate port).

use std::path::Path;

use sha2::{Digest, Sha256};

use mgc_formats::bundle::{BUNDLE_VERSION, BundleManifest, BundleSource, TerrainAtlasInfo};
use mgc_formats::{Game, Importer};

use crate::bake::BakeError;
use crate::gamedata::GameSource;
use crate::sprites;
use crate::tmaps::TmapsArchive;

/// Width of the baked sprite atlas; retail sprites max out well below.
/// Doubled as needed to keep the height under wgpu's baseline 2D
/// texture limit (MC2's animated sets pack ~9.4k rows at 1024).
const SPRITE_ATLAS_WIDTH: u32 = 1024;
const MAX_TEXTURE_DIM: u32 = 8192;
/// UI sprite atlas width: 87 small sprites (~122k pixels) pack well
/// under 256 rows at this width.
const UI_ATLAS_WIDTH: u32 = 512;

const SHADE_LUT_LEN: usize = 0x4000; // 64 shade levels x 256 colors
const TILE_COLORS_OFFSET: usize = 0x14000;
const TABLES_LEN: usize = 0x14600; // the engine's full color-table blob
const ATLAS_CELL: u32 = 32;
const ATLAS_WIDTH: u32 = 256;
const ATLAS_CELLS: u32 = 152;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One bundle variant: which original catalogs feed the uniform members.
/// The differences between MC1's world tilesets and MC2's environments
/// are entirely in file names and one TABLES layout shift, so a single
/// spec-driven bake covers both games.
struct VariantSpec {
    variant: &'static str,
    game: Game,
    /// 768-byte 6-bit VGA palette (`DATA/…`).
    palette: &'static str,
    /// 0x14600-byte color-table blob (`DATA/…`).
    tables: &'static str,
    /// Offset of the 64x256 shade LUT inside the tables blob: MC1 keeps
    /// it at +0x0000; MC2 keeps a pixel-remap table there and the shade
    /// LUT at +0x4000 (remc2 Basic.cpp:123, GameRenderNG shading paths).
    /// The tile-type→map-color table is at +0x14000 in both games.
    shade_offset: usize,
    /// Terrain atlas, 256px wide, 152 cells of 32x32; the terrain-type
    /// byte is the cell index in both games (identity mapping, remc2
    /// GameRenderHD.cpp:854).
    atlas: &'static str,
    /// TMAPS base name without extension (`DATA/…`): world billboards.
    tmaps: &'static str,
    /// MC1 only: BUILD base name; implies `SEARCH.DAT` too. MC2's
    /// terrain-feature pass is a separate original implementation whose
    /// data semantics are unverified — omitted until that port.
    build: Option<&'static str>,
    /// UI sprite library base name (HSPR = the 640x480 set; see
    /// `crate::hspr`). Implies `DATA/BOOK.PAL` (the book screen's own
    /// palette) for MC1. MC2's per-environment HSPR{D,N,C} wait for
    /// its UI track.
    ui: Option<&'static str>,
}

const MC1_VARIANTS: [VariantSpec; 2] = [
    VariantSpec {
        variant: "mc1-temperate",
        game: Game::MagicCarpet1,
        palette: "DATA/PAL0-0.DAT",
        tables: "DATA/TABLES.DAT",
        shade_offset: 0,
        atlas: "DATA/BLK0-1.DAT",
        tmaps: "DATA/TMAPS0-0",
        build: Some("DATA/BUILD0-0"),
        ui: Some("DATA/HSPR0-0"),
    },
    VariantSpec {
        variant: "mc1-arctic",
        game: Game::MagicCarpet1,
        palette: "DATA/PAL1-0.DAT",
        tables: "DATA/DTABLES.DAT",
        shade_offset: 0,
        atlas: "DATA/BLK1-1.DAT",
        tmaps: "DATA/TMAPS1-0",
        build: Some("DATA/BUILD1-0"),
        ui: Some("DATA/HSPR1-0"),
    },
];

/// MC2's per-environment catalogs (remc2 ReadAndDecompress.cpp:21-170,
/// Level.cpp:878-906): day uses the un-suffixed BLOCK32 atlas, night
/// splits into plain and "fog" graphics on the level header's gfx_type
/// bit 1 (fog swaps atlas + palette; tables and TMAPS stay night), and
/// TMAPS digits are the MapType ordinals (0 day / 1 night / 2 cave).
const MC2_VARIANTS: [VariantSpec; 4] = [
    VariantSpec {
        variant: "mc2-day",
        game: Game::MagicCarpet2,
        palette: "DATA/PALD-0.DAT",
        tables: "DATA/TABLESD.DAT",
        shade_offset: MC2_SHADE_OFFSET,
        atlas: "DATA/BLOCK32.DAT",
        tmaps: "DATA/TMAPS0-0",
        build: None,
        ui: None,
    },
    VariantSpec {
        variant: "mc2-night",
        game: Game::MagicCarpet2,
        palette: "DATA/PALN-0.DAT",
        tables: "DATA/TABLESN.DAT",
        shade_offset: MC2_SHADE_OFFSET,
        atlas: "DATA/BL32N0-0.DAT",
        tmaps: "DATA/TMAPS1-0",
        build: None,
        ui: None,
    },
    VariantSpec {
        variant: "mc2-night-fog",
        game: Game::MagicCarpet2,
        palette: "DATA/PALF-0.DAT",
        tables: "DATA/TABLESN.DAT",
        shade_offset: MC2_SHADE_OFFSET,
        atlas: "DATA/BL32F0-0.DAT",
        tmaps: "DATA/TMAPS1-0",
        build: None,
        ui: None,
    },
    VariantSpec {
        variant: "mc2-cave",
        game: Game::MagicCarpet2,
        palette: "DATA/PALC-0.DAT",
        tables: "DATA/TABLESC.DAT",
        shade_offset: MC2_SHADE_OFFSET,
        atlas: "DATA/BL32C0-0.DAT",
        tmaps: "DATA/TMAPS2-0",
        build: None,
        ui: None,
    },
];

const MC2_SHADE_OFFSET: usize = 0x4000;

fn bake_bundle_set(
    src: &GameSource,
    out_dir: &Path,
    specs: &[VariantSpec],
) -> Result<Vec<(String, String)>, BakeError> {
    let mut outputs = Vec::new();
    for spec in specs {
        let dir = out_dir.join("assets").join(spec.variant);
        std::fs::create_dir_all(&dir).map_err(|e| BakeError::Io(dir.clone(), e))?;
        let baked = bake_variant(src, &dir, spec)?;
        outputs.extend(
            baked
                .into_iter()
                .map(|(name, sha)| (format!("assets/{}/{name}", spec.variant), sha)),
        );
    }
    Ok(outputs)
}

/// Bake MC1's two world tilesets into `out_dir/assets/{mc1-temperate,
/// mc1-arctic}`. Returns `(manifest path, sha256)` pairs for every
/// written member. Member semantics are in docs/FORMAT.md "Asset
/// bundles"; per-variant source catalogs in [`MC1_VARIANTS`].
pub fn bake_mc1_bundles(
    src: &GameSource,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    let outputs = bake_bundle_set(src, out_dir, &MC1_VARIANTS)?;
    // The pre-bundle asset layout; remove so stale files cannot shadow
    // the bundles.
    let legacy = out_dir.join("mc1/assets");
    if legacy.is_dir() {
        std::fs::remove_dir_all(&legacy).map_err(|e| BakeError::Io(legacy.clone(), e))?;
    }
    Ok(outputs)
}

/// Bake MC2's four environment bundles (`mc2-day`, `mc2-night`,
/// `mc2-night-fog`, `mc2-cave`) from the CD catalogs. No search/build
/// members yet — MC2's terrain-feature pass is a separate port.
pub fn bake_mc2_bundles(
    src: &GameSource,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    bake_bundle_set(src, out_dir, &MC2_VARIANTS)
}

fn bake_variant(
    src: &GameSource,
    dir: &Path,
    spec: &VariantSpec,
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
    // (several catalogs and the TMAPS TABs ship raw).
    let source = |rel: &str, sources: &mut Vec<BundleSource>| -> Result<Vec<u8>, BakeError> {
        let raw = src
            .read(rel)
            .map_err(|e| BakeError::Io(Path::new(rel).to_path_buf(), e))?;
        sources.push(BundleSource {
            file: rel.rsplit('/').next().unwrap_or(rel).to_string(),
            sha256: hex(&Sha256::digest(&raw)),
        });
        if crate::rnc::is_rnc(&raw) {
            crate::rnc::decompress(&raw)
                .map_err(|e| BakeError::Level(Path::new(rel).to_path_buf(), 0, e.to_string()))
        } else {
            Ok(raw)
        }
    };
    let expect = |rel: &str, data: &[u8], len: usize| -> Result<(), BakeError> {
        if data.len() != len {
            return Err(BakeError::Level(
                Path::new(rel).to_path_buf(),
                0,
                format!("{} bytes, expected {len}", data.len()),
            ));
        }
        Ok(())
    };

    // Palette: 6-bit VGA -> RGBA8, index 0 transparent.
    let vga = source(spec.palette, &mut sources)?;
    expect(spec.palette, &vga, 768)?;
    let mut rgba = Vec::with_capacity(1024);
    for (i, c) in vga.chunks_exact(3).enumerate() {
        for &v in c {
            rgba.push((v << 2) | (v >> 4));
        }
        rgba.push(if i == 0 { 0 } else { 255 });
    }
    emit("palette.bin", &rgba)?;

    // Color tables: shade LUT at the game's offset (see
    // VariantSpec::shade_offset), tile-type→map-color at +0x14000 in
    // both games.
    let tables = source(spec.tables, &mut sources)?;
    expect(spec.tables, &tables, TABLES_LEN)?;
    emit(
        "shade-lut.bin",
        &tables[spec.shade_offset..spec.shade_offset + SHADE_LUT_LEN],
    )?;
    emit(
        "tile-colors.bin",
        &tables[TILE_COLORS_OFFSET..TILE_COLORS_OFFSET + 256],
    )?;

    // Terrain atlas.
    let atlas = source(spec.atlas, &mut sources)?;
    expect(
        spec.atlas,
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
    let tmaps_dat_file = format!("{}.DAT", spec.tmaps);
    let tmaps_dat = source(&tmaps_dat_file, &mut sources)?;
    let tmaps_tab = source(&format!("{}.TAB", spec.tmaps), &mut sources)?;
    let archive = TmapsArchive::open(&tmaps_dat, &tmaps_tab).map_err(|e| {
        BakeError::Level(Path::new(&tmaps_dat_file).to_path_buf(), 0, e.to_string())
    })?;
    let (decoded, warnings) = sprites::decode_tmaps(&archive).map_err(|e| {
        BakeError::Level(Path::new(&tmaps_dat_file).to_path_buf(), 0, e.to_string())
    })?;
    for w in warnings {
        eprintln!("note: {}: {w}", spec.variant);
    }
    let mut atlas_width = SPRITE_ATLAS_WIDTH;
    let mut packed = sprites::pack(&decoded, atlas_width);
    while packed.index.atlas_height > MAX_TEXTURE_DIM && atlas_width < MAX_TEXTURE_DIM {
        atlas_width *= 2;
        packed = sprites::pack(&decoded, atlas_width);
    }
    emit("sprites.bin", &packed.atlas)?;
    emit(
        "sprites.json",
        &serde_json::to_vec_pretty(&packed.index).expect("sprite index serializes"),
    )?;

    // Terrain-feature data (MC1 only for now, see VariantSpec::build).
    if let Some(build) = spec.build {
        let tab_file = format!("{build}.TAB");
        let tab = source(&tab_file, &mut sources)?;
        if tab.len() % 6 != 0 {
            return Err(BakeError::Level(
                Path::new(&tab_file).to_path_buf(),
                0,
                format!("{} bytes is not 6-byte entries", tab.len()),
            ));
        }
        emit("build.tab.bin", &tab)?;
        let build_dat = source(&format!("{build}.DAT"), &mut sources)?;
        emit("build.dat.bin", &build_dat)?;
        let search = source("DATA/SEARCH.DAT", &mut sources)?;
        expect("DATA/SEARCH.DAT", &search, 1024)?;
        emit("search.bin", &search)?;
    }

    // UI sprites (HSPR) + the book screen palette (MC1 only for now).
    if let Some(ui) = spec.ui {
        let dat_file = format!("{ui}.DAT");
        let dat = source(&dat_file, &mut sources)?;
        let tab = source(&format!("{ui}.TAB"), &mut sources)?;
        let decoded = crate::hspr::decode(&dat, &tab)
            .map_err(|e| BakeError::Level(Path::new(&dat_file).to_path_buf(), 0, e.to_string()))?;
        let packed = sprites::pack(&decoded, UI_ATLAS_WIDTH);
        emit("ui-sprites.bin", &packed.atlas)?;
        emit(
            "ui-sprites.json",
            &serde_json::to_vec_pretty(&packed.index).expect("ui sprite index serializes"),
        )?;

        // The UI blend LUT: TABLES.DAT's middle 64KB (+0x4000..+0x14000),
        // the slice between the shade LUT and the map colors. The
        // original's 2D blits resolve every pixel as
        // `blend[src | dest<<8]` (remc1 strPal.byte_BB934_BB924,
        // sub_main.cpp:27444/27564) — spell icons composite through it
        // against the book page, which is where their true colors
        // (e.g. the red heal heart) come from; raw icon indices are a
        // ramp that reads garish under any palette directly.
        emit("blend-lut.bin", &tables[SHADE_LUT_LEN..TILE_COLORS_OFFSET])?;

        let book = source("DATA/BOOK.PAL", &mut sources)?;
        expect("DATA/BOOK.PAL", &book, 768)?;
        let mut rgba = Vec::with_capacity(1024);
        for (i, c) in book.chunks_exact(3).enumerate() {
            for &v in c {
                rgba.push((v << 2) | (v >> 4));
            }
            rgba.push(if i == 0 { 0 } else { 255 });
        }
        emit("book-palette.bin", &rgba)?;
    }

    let manifest = BundleManifest {
        format_version: BUNDLE_VERSION,
        variant: spec.variant.to_string(),
        game: spec.game,
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
