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
//! `mc2-cave` — same schema; since Phase 3 they carry `search.bin`
//! (same format as MC1) and `bldgprm.bin` (MC2's building-parameter
//! table) instead of MC1's BUILD members.

use std::path::Path;

use sha2::{Digest, Sha256};

use mgc_formats::bundle::{
    BUNDLE_VERSION, BundleManifest, BundleSource, MusicIndex, MusicTrack, SpeechClip, SpeechIndex,
    TerrainAtlasInfo,
};
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
    /// Offset of the 64x256 shade LUT inside the tables blob: +0x0000
    /// in BOTH games (row 32 ≈ identity, row 0 = the fog/sky color,
    /// row 63 = black). +0x4000 is the 256x256 sprite BLEND matrix
    /// (`T[0x4000 + (src<<8)|dst]`, remc2 GameRenderObjects
    /// DrawSprite_41BD3) — see docs/traces/mc2-transparency-drawlist.md,
    /// which corrected the earlier "+0x4000 shade" misread. The
    /// tile-type→map-color table is at +0x14000 in both games.
    shade_offset: usize,
    /// Terrain atlas, 256px wide, 152 cells of 32x32; the terrain-type
    /// byte is the cell index in both games (identity mapping, remc2
    /// GameRenderHD.cpp:854).
    atlas: &'static str,
    /// TMAPS base name without extension (`DATA/…`): world billboards.
    tmaps: &'static str,
    /// Ring search-order table (`DATA/SEARCH.DAT`, 1024 bytes) — the
    /// same 32x32 relative-offset format in BOTH games (remc2 loads it
    /// via sub_101C0, EventsFunctions.cpp:3589).
    search: Option<&'static str>,
    /// BUILD bank base name (6-byte .TAB rows + .DAT cells — building
    /// footprints/paint). MC1: 1-byte cell codes; MC2 (`BUILD0-0`,
    /// ONE bank for all environments — remc2 Basic.cpp:271 loads a
    /// fixed path): 2 bytes per cell {paint code, pad height}, read
    /// by the build action (ApplyTerrainModification_37240 :27181).
    build: Option<&'static str>,
    /// MC2 only: the building-parameter table (`DATA/BLDGPRM.DAT`,
    /// 4-byte records {u16 production rate, u8 flags, u8 chain};
    /// loader remc2 sub_539A0 EventsFunctions.cpp:38319 — flags:
    /// 0x10 GenerateEvents pass F/G split, 8 no mana/production,
    /// 4 no cave second-heightmap, 1 enterable). Footprint sizes
    /// live in the BUILD bank, NOT here.
    bldgprm: Option<&'static str>,
    /// MC2 only: the spell table (`DATA/SPELLS.DAT`, 26 rows x 80
    /// bytes: {i8, u8 enabled, 3 x 26-byte subspell tiers} — remc2
    /// Spells.h + Basic.cpp:334 loads it over the Spells.cpp baked-in
    /// fallback; the retail CD values DIFFER from that fallback, so
    /// carrying the real file is load-bearing). Feeds the par1-authored
    /// class-10 effect overrides and the class-15 cast costs.
    spells: Option<&'static str>,
    /// UI sprite library base name (HSPR = the 640x480 set; see
    /// `crate::hspr`). Implies `DATA/BOOK.PAL` (the book screen's own
    /// palette) for MC1 only — MC2 has no book screen. MC2's TAB/DAT
    /// pairs are the same self-describing format (remc2
    /// `bitmap_pos_struct2_t`, portability/bitmap_pos_struct.h:27 —
    /// {u32 offset, u8 w, u8 h}; same signed-RLE rows).
    ui: Option<&'static str>,
    /// The messaging/notification bitmap font base name (same HSPR
    /// TAB/DAT format, decoded by `crate::hspr`). BOTH games render the
    /// top-of-screen notification with the small `DATA/FONT1` (~4x7):
    /// `FontType_D419D` is only ever 1 or 3 and the font table
    /// `E9B20[4] = {FONT0, FONT1, HFONT3, FONT1}` maps both to FONT1
    /// (remc2 Basic.cpp:241/324; remc1 sub_main.cpp `sub_5A3C0(1)`) —
    /// HFONT3 is never on this path. Glyphs are 1-bit masks (every ink
    /// pixel = index 1); the sprite id for ASCII char `c` is `c + 1`
    /// (id 0 null, id 33 = space). Baked to `font.bin`/`font.json` —
    /// the app appends the glyph masks to its UI atlas as white and
    /// tints per DrawText's `color` argument.
    font: Option<&'static str>,
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
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD0-0"),
        bldgprm: None,
        spells: None,
        ui: Some("DATA/HSPR0-0"),
        font: Some("DATA/FONT1"),
    },
    VariantSpec {
        variant: "mc1-arctic",
        game: Game::MagicCarpet1,
        palette: "DATA/PAL1-0.DAT",
        tables: "DATA/DTABLES.DAT",
        shade_offset: 0,
        atlas: "DATA/BLK1-1.DAT",
        tmaps: "DATA/TMAPS1-0",
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD1-0"),
        bldgprm: None,
        spells: None,
        ui: Some("DATA/HSPR1-0"),
        font: Some("DATA/FONT1"),
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
        shade_offset: 0,
        atlas: "DATA/BLOCK32.DAT",
        tmaps: "DATA/TMAPS0-0",
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD0-0"),
        bldgprm: Some("DATA/BLDGPRM.DAT"),
        spells: Some("DATA/SPELLS.DAT"),
        ui: Some("DATA/HSPRD0-0"),
        font: Some("DATA/FONT1"),
    },
    VariantSpec {
        variant: "mc2-night",
        game: Game::MagicCarpet2,
        palette: "DATA/PALN-0.DAT",
        tables: "DATA/TABLESN.DAT",
        shade_offset: 0,
        atlas: "DATA/BL32N0-0.DAT",
        tmaps: "DATA/TMAPS1-0",
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD0-0"),
        bldgprm: Some("DATA/BLDGPRM.DAT"),
        spells: Some("DATA/SPELLS.DAT"),
        ui: Some("DATA/HSPRN0-0"),
        font: Some("DATA/FONT1"),
    },
    VariantSpec {
        variant: "mc2-night-fog",
        game: Game::MagicCarpet2,
        palette: "DATA/PALF-0.DAT",
        tables: "DATA/TABLESN.DAT",
        shade_offset: 0,
        atlas: "DATA/BL32F0-0.DAT",
        tmaps: "DATA/TMAPS1-0",
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD0-0"),
        bldgprm: Some("DATA/BLDGPRM.DAT"),
        spells: Some("DATA/SPELLS.DAT"),
        ui: Some("DATA/HSPRN0-0"),
        font: Some("DATA/FONT1"),
    },
    VariantSpec {
        variant: "mc2-cave",
        game: Game::MagicCarpet2,
        palette: "DATA/PALC-0.DAT",
        tables: "DATA/TABLESC.DAT",
        shade_offset: 0,
        atlas: "DATA/BL32C0-0.DAT",
        tmaps: "DATA/TMAPS2-0",
        search: Some("DATA/SEARCH.DAT"),
        build: Some("DATA/BUILD0-0"),
        bldgprm: Some("DATA/BLDGPRM.DAT"),
        spells: Some("DATA/SPELLS.DAT"),
        ui: Some("DATA/HSPRC0-0"),
        font: Some("DATA/FONT1"),
    },
];

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

/// Highest of MC1's free-RAM sample-quality tiers (`SNDS<bank>-1`),
/// 22050 Hz; `-0` is the same audio at half rate, `-3` lower still
/// (see `crate::sound`).
const MC1_SOUND_QUALITY: u32 = 1;
const MC1_SOUND_RATE: u32 = 22050;
/// OPL render / redbook output rate.
const MUSIC_RATE: u32 = 44100;
/// MC1 ships sample banks 0..=13 (bank = per-level/screen sound set).
const MC1_SOUND_BANKS: std::ops::RangeInclusive<u32> = 0..=13;

/// Bake MC1's audio bundle (`baked/assets/mc1-audio/`): every SNDS
/// sample bank at the highest quality tier, deduplicated into one PCM
/// blob. Sounds and music are tileset-independent (the bank digit is a
/// level selector, not a world-set pair), so audio is one per-game
/// bundle rather than a member of the graphics variants.
pub fn bake_mc1_audio(
    src: &GameSource,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    let dir = out_dir.join("assets").join("mc1-audio");
    std::fs::create_dir_all(&dir).map_err(|e| BakeError::Io(dir.clone(), e))?;

    let mut outputs = Vec::new();
    let mut sources = Vec::new();

    let mut emit = |name: &str, bytes: &[u8]| -> Result<(), BakeError> {
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| BakeError::Io(path, e))?;
        outputs.push((name.to_string(), hex(&Sha256::digest(bytes))));
        Ok(())
    };
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

    // Sample banks. Decompressed DAT/TAB pairs are borrowed by the
    // parsed banks, so decompress all of them first.
    let mut raw_pairs = Vec::new();
    for bank in MC1_SOUND_BANKS {
        let dat_rel = format!("DATA/SNDS{bank}-{MC1_SOUND_QUALITY}.DAT");
        let tab_rel = format!("DATA/SNDS{bank}-{MC1_SOUND_QUALITY}.TAB");
        if !src.exists(&dat_rel) {
            continue;
        }
        let dat = source(&dat_rel, &mut sources)?;
        let tab = source(&tab_rel, &mut sources)?;
        raw_pairs.push((bank, dat_rel, tab, dat));
    }
    let mut banks = Vec::new();
    for (bank, dat_rel, tab, dat) in &raw_pairs {
        banks.push(
            crate::sound::parse_bank(*bank, tab, dat, true)
                .map_err(|e| BakeError::Level(Path::new(dat_rel).to_path_buf(), 0, e))?,
        );
    }
    let (index, blob) = crate::sound::bake_blob(&banks, MC1_SOUND_RATE);
    emit("sounds.bin", &blob)?;
    emit(
        "sounds.json",
        &serde_json::to_vec_pretty(&index).expect("sound index serializes"),
    )?;

    // Music: the AdLib arrangement (`MUSIC<bank>-0`, the `-0` driver
    // digit is AdLib per remc1 :54030 — 0xA002 loads inst/drum.bnk)
    // rendered through OPL3 with the game's own banks, FLAC per song.
    // When the host can render General MIDI (fluidsynth + a GM
    // soundfont, see `crate::fluid`), the `-2` arrangement (`GENERAL`,
    // remc1 :54029-30 — 0xA001 → digit 2) is baked alongside as the
    // optional GM upgrade; absent hosts still get the full FM bundle.
    let music_dir = dir.join("music");
    std::fs::create_dir_all(&music_dir).map_err(|e| BakeError::Io(music_dir.clone(), e))?;
    let inst = crate::adlib::parse_bnk(&source("DATA/INST.BNK", &mut sources)?)
        .map_err(|e| BakeError::Level(Path::new("DATA/INST.BNK").to_path_buf(), 0, e))?;
    let drum = crate::adlib::parse_bnk(&source("DATA/DRUM.BNK", &mut sources)?)
        .map_err(|e| BakeError::Level(Path::new("DATA/DRUM.BNK").to_path_buf(), 0, e))?;
    let gm = match crate::fluid::GmRenderer::locate() {
        Ok(r) => Some(r),
        Err(why) => {
            println!("note: mc1 music: no GM render ({why}) — FM only");
            None
        }
    };
    let mut music = MusicIndex { tracks: Vec::new() };
    for bank in 0..=1u32 {
        let dat_rel = format!("DATA/MUSIC{bank}-0.DAT");
        let tab_rel = format!("DATA/MUSIC{bank}-0.TAB");
        if !src.exists(&dat_rel) {
            continue;
        }
        let dat = source(&dat_rel, &mut sources)?;
        let tab = source(&tab_rel, &mut sources)?;
        let parsed = crate::sound::parse_bank(bank, &tab, &dat, false)
            .map_err(|e| BakeError::Level(Path::new(&dat_rel).to_path_buf(), 0, e))?;
        // The GM arrangement, keyed by song stem (`cgame1.gen` ↔
        // `cgame1.hmp` — same songs, per-driver patches/mix).
        let mut gm_songs: Vec<(String, crate::hmp::Song)> = Vec::new();
        if gm.is_some() && src.exists(&format!("DATA/MUSIC{bank}-2.DAT")) {
            let dat = source(&format!("DATA/MUSIC{bank}-2.DAT"), &mut sources)?;
            let tab = source(&format!("DATA/MUSIC{bank}-2.TAB"), &mut sources)?;
            let parsed = crate::sound::parse_bank(bank, &tab, &dat, false)
                .map_err(|e| BakeError::Level(Path::new("DATA/MUSIC-2.DAT").to_path_buf(), 0, e))?;
            for (_, name, bytes) in &parsed.entries {
                let stem = name.split('.').next().unwrap_or(name).to_string();
                let song = crate::hmp::parse(bytes).map_err(|e| {
                    BakeError::Level(
                        Path::new("DATA/MUSIC-2.DAT").to_path_buf(),
                        0,
                        format!("{name}: {e}"),
                    )
                })?;
                gm_songs.push((stem, song));
            }
        }
        for (_, name, hmp_bytes) in &parsed.entries {
            let err = |e: String| {
                BakeError::Level(Path::new(&dat_rel).to_path_buf(), 0, format!("{name}: {e}"))
            };
            let song = crate::hmp::parse(hmp_bytes).map_err(err)?;
            // In-game songs keep their danger layers (MIDI channels
            // 3/4/5, CC7-0-muted, runtime-faded by the original) as a
            // separate sample-aligned stem; the base file is the
            // ambient mix.
            let layered = crate::adlib::has_danger_layer(&song);
            let mix = if layered {
                crate::adlib::MixSpec::ambient()
            } else {
                crate::adlib::MixSpec::full()
            };
            let pcm = crate::adlib::render(&song, &inst, &drum, MUSIC_RATE, &mix).map_err(err)?;
            let flac = crate::flac::encode(&pcm, 1, MUSIC_RATE).map_err(err)?;
            let name = name.strip_suffix(".hmp").unwrap_or(name);
            let member = format!("music/{bank}-{name}.flac");
            emit(&member, &flac)?;
            let danger_file = if layered {
                let stem = crate::adlib::render(
                    &song,
                    &inst,
                    &drum,
                    MUSIC_RATE,
                    &crate::adlib::MixSpec::danger_stem(),
                )
                .map_err(err)?;
                debug_assert_eq!(stem.len(), pcm.len(), "stems must stay sample-aligned");
                let flac = crate::flac::encode(&stem, 1, MUSIC_RATE).map_err(err)?;
                let member = format!("music/{bank}-{name}-danger.flac");
                emit(&member, &flac)?;
                Some(member)
            } else {
                None
            };
            // The GM upgrade: same song from the `-2` arrangement,
            // fluidsynth-rendered ambient + danger stem (both scaled
            // by ONE factor — the overlay sum is what must not clip).
            let mut gm_file = None;
            let mut gm_danger_file = None;
            if let (Some(renderer), Some((_, gm_song))) =
                (gm.as_ref(), gm_songs.iter().find(|(stem, _)| stem == name))
            {
                let render = |mix: &crate::adlib::MixSpec, tag: &str| {
                    let midi = crate::smf::encode(gm_song, mix);
                    renderer.render(
                        &midi,
                        MUSIC_RATE,
                        &music_dir,
                        &format!("{bank}-{name}-{tag}"),
                    )
                };
                let layered = crate::adlib::has_danger_layer(gm_song);
                let base_mix = if layered {
                    crate::adlib::MixSpec::ambient()
                } else {
                    crate::adlib::MixSpec::full()
                };
                let mut base = render(&base_mix, "base").map_err(err)?;
                let mut stem = if layered {
                    Some(render(&crate::adlib::MixSpec::danger_stem(), "danger").map_err(err)?)
                } else {
                    None
                };
                let frames = base.len().max(stem.as_ref().map_or(0, Vec::len));
                base.resize(frames, 0.0);
                let mut peak = 0f32;
                if let Some(stem) = &mut stem {
                    stem.resize(frames, 0.0);
                    for (b, s) in base.iter().zip(stem.iter()) {
                        peak = peak.max((b + s).abs());
                    }
                } else {
                    for b in &base {
                        peak = peak.max(b.abs());
                    }
                }
                let scale = if peak > 0.0 { 30000.0 / peak } else { 1.0 };
                let quantize = |pcm: &[f32]| -> Vec<i16> {
                    pcm.iter()
                        .map(|s| (s * scale).clamp(-32767.0, 32767.0) as i16)
                        .collect()
                };
                let member = format!("music/{bank}-{name}-gm.flac");
                emit(
                    &member,
                    &crate::flac::encode(&quantize(&base), 2, MUSIC_RATE).map_err(err)?,
                )?;
                gm_file = Some(member);
                if let Some(stem) = &stem {
                    let member = format!("music/{bank}-{name}-gm-danger.flac");
                    emit(
                        &member,
                        &crate::flac::encode(&quantize(stem), 2, MUSIC_RATE).map_err(err)?,
                    )?;
                    gm_danger_file = Some(member);
                }
            }
            music.tracks.push(MusicTrack {
                bank,
                name: name.to_string(),
                file: member,
                danger_file,
                gm_file,
                gm_danger_file,
                source: format!("MUSIC{bank}-0 {}.HMP", name.to_ascii_uppercase()),
            });
        }
    }
    emit(
        "music.json",
        &serde_json::to_vec_pretty(&music).expect("music index serializes"),
    )?;

    let manifest = BundleManifest {
        format_version: BUNDLE_VERSION,
        bake_epoch: mgc_formats::BAKE_EPOCH,
        variant: "mc1-audio".to_string(),
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
    Ok(outputs
        .into_iter()
        .map(|(name, sha)| (format!("assets/mc1-audio/{name}"), sha))
        .collect())
}

/// Bake MC2's audio bundle (`baked/assets/mc2-audio/`).
///
/// Three member families (traces docs/traces/mc2-music-law.md,
/// mc2-music-dat-xmi.md, mc2-voiceover-triggers.md):
/// - samples: `SOUND/SOUND.DAT`, best shipped tier;
/// - music: the `SOUND/MUSIC.DAT` GM bank-0 XMI sub-songs (the "C2" =
///   Magic Carpet 2 set — GAME1/2/3 = the MapType tracks Night/Day/
///   Cave, SETUP = menu) rendered through fluidsynth — retail gameplay
///   music is NEVER the redbook, and bank 0 (not the `-music2` bank-1
///   "C1"/MC1 set) is the default. cc119-tagged channels are the
///   war/danger layers
///   (expression-zeroed in peace, combat-ramped — Sound.cpp:851/
///   5880), baked as the MC1-style ambient mix + sample-aligned
///   danger stem;
/// - speech: the redbook voiceover pre-sliced by `CdTracks_DB080`
///   (table row r = level r → rip track r+2; row 27 = dead data) —
///   the runtime plays whole clips, never seeks inside a track.
pub fn bake_mc2_audio(
    src: &GameSource,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    let Some(image) = src.cd_image() else {
        return Ok(Vec::new());
    };
    let image = image.to_path_buf();
    let cue_path = image.with_extension("ins");
    let cue = match std::fs::read_to_string(&cue_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "note: mc2: no cue sheet at {} — skipping redbook rip",
                cue_path.display()
            );
            return Ok(Vec::new());
        }
    };

    let dir = out_dir.join("assets").join("mc2-audio");
    let music_dir = dir.join("music");
    std::fs::create_dir_all(&music_dir).map_err(|e| BakeError::Io(music_dir.clone(), e))?;

    let mut outputs = Vec::new();
    let mut emit = |name: &str, bytes: &[u8]| -> Result<(), BakeError> {
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| BakeError::Io(path, e))?;
        outputs.push((name.to_string(), hex(&Sha256::digest(bytes))));
        Ok(())
    };

    let mut sources = Vec::new();

    // Samples: SOUND/SOUND.DAT, best shipped quality tier (8-bit
    // 22050 across the retail GOG file — same PCM encoding as MC1).
    let sound_dat_raw = src
        .read("SOUND/SOUND.DAT")
        .map_err(|e| BakeError::Io(Path::new("SOUND/SOUND.DAT").to_path_buf(), e))?;
    sources.push(BundleSource {
        file: "SOUND.DAT".into(),
        sha256: hex(&Sha256::digest(&sound_dat_raw)),
    });
    let banks = crate::sound::parse_mc2_sound_dat(&sound_dat_raw)
        .map_err(|e| BakeError::Level(Path::new("SOUND/SOUND.DAT").to_path_buf(), 0, e))?;
    let (index, blob) = crate::sound::bake_blob(&banks, MC1_SOUND_RATE);
    emit("sounds.bin", &blob)?;
    emit(
        "sounds.json",
        &serde_json::to_vec_pretty(&index).expect("sound index serializes"),
    )?;

    // Gameplay music: MUSIC.DAT GM bank 0 (the "C2" = Magic Carpet 2
    // set), sub-songs 0..=3 by role. Bank 0 is the DEFAULT gameplay
    // bank: `musicChannel_E3814 = 0` (Sound.cpp:49, never reassigned)
    // → `InitMusic_8D970` loads `InitMusicBank(0)` (Sound.cpp:801).
    // Bank 1 (the "C1" = Magic Carpet 1 set) loads ONLY under the
    // hidden `-music2` command-line flag (EF:39191/43023 guarded by
    // `setting_byte4_25 & 0x40`, default clear) — the classic-MC1-
    // soundtrack alternate, a future opt-in (authenticity matrix), NOT
    // the default. (Baking bank 1 was the "wrong/unfamiliar gameplay
    // tracks, aggressive cave" bug — docs/traces/mc2-music-law.md.)
    // Requires the GM renderer — MC2 has no pure-Rust FM fallback yet
    // (the F section is a future faithful-alternate).
    let mut music = MusicIndex { tracks: Vec::new() };
    let music_dat = src
        .read("SOUND/MUSIC.DAT")
        .map_err(|e| BakeError::Io(Path::new("SOUND/MUSIC.DAT").to_path_buf(), e))?;
    sources.push(BundleSource {
        file: "MUSIC.DAT".into(),
        sha256: hex(&Sha256::digest(&music_dat)),
    });
    match crate::fluid::GmRenderer::locate() {
        Err(why) => println!("note: mc2 music: no GM render ({why}) — music skipped"),
        Ok(renderer) => {
            let subsongs = crate::mc2_music::parse_gm_bank(&music_dat, 0)
                .map_err(|e| BakeError::Level(Path::new("SOUND/MUSIC.DAT").to_path_buf(), 0, e))?;
            // MapType track n → sub-song n-1 (the ±1 lives in AIL:
            // `SOUND_start_sequence(track-1)`, Sound.cpp:4974): Night=1
            // →GAME1, Day=2→GAME2, Cave=3→GAME3; menu StartMusic(4)→
            // SETUP (idx 3, the shared C2SETUP).
            const ROLES: [(usize, &str); 4] = [
                (0, "mc2-night"),
                (1, "mc2-day"),
                (2, "mc2-cave"),
                (3, "mc2-menu"),
            ];
            for (idx, role) in ROLES {
                let sub = subsongs.get(idx).ok_or_else(|| {
                    BakeError::Level(
                        Path::new("SOUND/MUSIC.DAT").to_path_buf(),
                        0,
                        format!("GM bank 1 has no sub-song {idx}"),
                    )
                })?;
                let err = |e: String| {
                    BakeError::Level(
                        Path::new("SOUND/MUSIC.DAT").to_path_buf(),
                        0,
                        format!("{}: {e}", sub.name),
                    )
                };
                let layered = sub.song.has_war_layer();
                let base_mix = if layered {
                    crate::xmi::Mix::Ambient
                } else {
                    crate::xmi::Mix::Full
                };
                let render = |mix: crate::xmi::Mix, tag: &str| {
                    let midi = crate::xmi::encode_smf(&sub.song, mix);
                    renderer.render(&midi, MUSIC_RATE, &music_dir, &format!("{role}-{tag}"))
                };
                let mut base = render(base_mix, "base").map_err(err)?;
                let mut stem = if layered {
                    Some(render(crate::xmi::Mix::WarStem, "danger").map_err(err)?)
                } else {
                    None
                };
                // One shared normalization factor per song — the
                // overlay SUM is what must not clip (MC1 GM contract).
                let frames = base.len().max(stem.as_ref().map_or(0, Vec::len));
                base.resize(frames, 0.0);
                let mut peak = 0f32;
                if let Some(stem) = &mut stem {
                    stem.resize(frames, 0.0);
                    for (b, s) in base.iter().zip(stem.iter()) {
                        peak = peak.max((b + s).abs());
                    }
                } else {
                    for b in &base {
                        peak = peak.max(b.abs());
                    }
                }
                let scale = if peak > 0.0 { 30000.0 / peak } else { 1.0 };
                let quantize = |pcm: &[f32]| -> Vec<i16> {
                    pcm.iter()
                        .map(|s| (s * scale).clamp(-32767.0, 32767.0) as i16)
                        .collect()
                };
                let member = format!("music/{role}.flac");
                emit(
                    &member,
                    &crate::flac::encode(&quantize(&base), 2, MUSIC_RATE).map_err(err)?,
                )?;
                let danger_file = match &stem {
                    Some(stem) => {
                        let member = format!("music/{role}-danger.flac");
                        emit(
                            &member,
                            &crate::flac::encode(&quantize(stem), 2, MUSIC_RATE).map_err(err)?,
                        )?;
                        Some(member)
                    }
                    None => None,
                };
                music.tracks.push(MusicTrack {
                    bank: 0,
                    name: role.to_string(),
                    file: member,
                    danger_file,
                    gm_file: None,
                    gm_danger_file: None,
                    source: format!("MUSIC.DAT G bank 0 {}", sub.name),
                });
            }
        }
    }
    emit(
        "music.json",
        &serde_json::to_vec_pretty(&music).expect("music index serializes"),
    )?;

    // Voiceover: slice each rip track by its CdTracks_DB080 row.
    let image_len = std::fs::metadata(&image)
        .map_err(|e| BakeError::Io(image.clone(), e))?
        .len();
    let tracks = crate::redbook::parse_cue(&cue, image_len / crate::redbook::SECTOR)
        .map_err(|e| BakeError::Level(cue_path.clone(), 0, e))?;
    let speech_dir = dir.join("speech");
    std::fs::create_dir_all(&speech_dir).map_err(|e| BakeError::Io(speech_dir.clone(), e))?;
    let mut speech = SpeechIndex { clips: Vec::new() };
    for (row, entry) in crate::cdtracks::CD_TRACKS.iter().enumerate() {
        // Table row r = level r → rip track r+2 (duration-fit proof
        // in the trace; row 27 implies track 29 = dead data).
        let rip_number = entry.track as u32 + 1;
        let Some(track) = tracks.iter().find(|t| t.number == rip_number) else {
            continue;
        };
        let pcm = crate::redbook::read_track(&image, *track)
            .map_err(|e| BakeError::Io(image.clone(), e))?;
        for (seg, &(start, len)) in entry.segments.iter().enumerate() {
            if len == 0 {
                continue; // empty slot — retail no-ops on length 0
            }
            let start_ms = crate::cdtracks::frames_to_ms(start);
            let len_ms = crate::cdtracks::frames_to_ms(len);
            let rate = u64::from(crate::redbook::RATE);
            let a = (u64::from(start_ms) * rate / 1000 * 2) as usize;
            let b = (u64::from(start_ms + len_ms) * rate / 1000 * 2) as usize;
            let (a, b) = (a.min(pcm.len()), b.min(pcm.len()));
            if a >= b {
                println!("note: mc2 speech: row {row} seg {seg} out of track — skipped");
                continue;
            }
            let flac = crate::flac::encode(&pcm[a..b], 2, crate::redbook::RATE).map_err(|e| {
                BakeError::Level(image.clone(), 0, format!("row {row} seg {seg}: {e}"))
            })?;
            let member = format!("speech/level-{row:02}-seg-{seg}.flac");
            emit(&member, &flac)?;
            speech.clips.push(SpeechClip {
                row: row as u32,
                segment: seg as u32,
                file: member,
                ms: len_ms,
                source: format!(
                    "redbook track {rip_number} @ {start_ms}..{}ms",
                    start_ms + len_ms
                ),
            });
        }
    }
    emit(
        "speech.json",
        &serde_json::to_vec_pretty(&speech).expect("speech index serializes"),
    )?;

    let manifest = BundleManifest {
        format_version: BUNDLE_VERSION,
        bake_epoch: mgc_formats::BAKE_EPOCH,
        variant: "mc2-audio".to_string(),
        game: Game::MagicCarpet2,
        importer: Importer {
            name: "mgc-import".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        sources: {
            sources.push(BundleSource {
                file: format!(
                    "{} (redbook tracks)",
                    image
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                // Hashing the 400 MB image per bake is
                // disproportionate; provenance is the cue sheet's
                // digest instead.
                sha256: hex(&Sha256::digest(cue.as_bytes())),
            });
            sources
        },
    };
    emit(
        "bundle.json",
        &serde_json::to_vec_pretty(&manifest).expect("manifest serializes"),
    )?;
    Ok(outputs
        .into_iter()
        .map(|(name, sha)| (format!("assets/mc2-audio/{name}"), sha))
        .collect())
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

    // Ring search-order table — the same 1024-byte 32x32 format in
    // both games (remc2 sub_101C0).
    if let Some(sp) = spec.search {
        let search = source(sp, &mut sources)?;
        expect(sp, &search, 1024)?;
        emit("search.bin", &search)?;
    }

    // Terrain-feature/building data, MC1 flavor (BUILD .TAB/.DAT).
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
    }

    // Building parameters, MC2 flavor (BLDGPRM 4-byte records; remc2
    // loads 76 records into a 77-slot table, sub_539A0 :38328).
    if let Some(bp) = spec.bldgprm {
        let bldgprm = source(bp, &mut sources)?;
        if bldgprm.len() % 4 != 0 {
            return Err(BakeError::Level(
                Path::new(bp).to_path_buf(),
                0,
                format!("{} bytes is not 4-byte records", bldgprm.len()),
            ));
        }
        emit("bldgprm.bin", &bldgprm)?;
    }

    // Spell table, MC2 flavor (SPELLS.DAT verbatim: 26 rows x 80
    // bytes — remc2 Spells.h layout, loaded by the Basic.cpp:334
    // Pathstruct over the source's baked-in fallback).
    if let Some(sp) = spec.spells {
        let spells = source(sp, &mut sources)?;
        expect(sp, &spells, 26 * 80)?;
        emit("spells.bin", &spells)?;
    }

    // UI sprites (HSPR) + the book screen palette (MC1 only — MC2 has
    // no book screen; its CTRL selector pane draws over the live
    // frame with the variant palette). MC2's blend LUT sits at the
    // same +0x4000..+0x14000 slice of its TABLES{D,N,C} (remc2
    // GameUI.cpp:525/1105: `tablesx[0x4000 + 256*dest + src]`).
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

        if spec.game == Game::MagicCarpet1 {
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
    }

    // Messaging/notification font (HFONT3/FONT2): the same HSPR TAB/DAT
    // format, decoded to single-frame glyph sprites and packed into one
    // atlas. The glyphs are 1-bit coverage masks (every ink pixel =
    // index 1); the app appends them to its UI atlas as white and tints
    // per DrawText's `color` argument, so no palette bakes here — the
    // atlas is palette-independent. Sprite id for ASCII char `c` = c+1.
    if let Some(font) = spec.font {
        let dat_file = format!("{font}.DAT");
        let dat = source(&dat_file, &mut sources)?;
        let tab = source(&format!("{font}.TAB"), &mut sources)?;
        let decoded = crate::hspr::decode(&dat, &tab)
            .map_err(|e| BakeError::Level(Path::new(&dat_file).to_path_buf(), 0, e.to_string()))?;
        let packed = sprites::pack(&decoded, UI_ATLAS_WIDTH);
        emit("font.bin", &packed.atlas)?;
        emit(
            "font.json",
            &serde_json::to_vec_pretty(&packed.index).expect("font index serializes"),
        )?;
    }

    let manifest = BundleManifest {
        format_version: BUNDLE_VERSION,
        bake_epoch: mgc_formats::BAKE_EPOCH,
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
