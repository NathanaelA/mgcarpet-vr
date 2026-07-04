//! Baking: original archives in, `.mgcl` packages out.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::dattab::Archive;
use crate::level_mc1::{Mc1Level, ThingKind as Mc1Kind};
use crate::level_mc2::{MC2_LEVEL_SIZE, MapType as Mc2MapType, Mc2Level};
use mgc_formats::{
    FORMAT_VERSION, Game, GenParams, Importer, LevelHeader, LevelPackage, MapType, Meta, Source,
    StageCheckpoint, StageVar, Stages, TERRAIN_GRID_BYTES, Terrain, Thing, ThingKind, Things,
    WizardConfig, Wizards, mgcl,
};

#[derive(Debug)]
pub enum BakeError {
    Io(PathBuf, std::io::Error),
    Archive(PathBuf, crate::dattab::TabError),
    Level(PathBuf, u32, String),
    Write(PathBuf, mgcl::MgclError),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Archive(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Level(p, i, e) => write!(f, "{} level {i}: {e}", p.display()),
            Self::Write(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for BakeError {}

fn importer() -> Importer {
    Importer {
        name: "mgc-import".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Convert one parsed MC1/Hidden Worlds level into a package.
pub fn package_mc1_level(
    game: Game,
    level_index: u32,
    level: &Mc1Level,
    source: Source,
) -> LevelPackage {
    let mut things = Vec::new();
    for (slot, t) in level.things.iter().enumerate() {
        let kind = match t.kind() {
            Mc1Kind::Entity => ThingKind::Entity,
            Mc1Kind::Marker => ThingKind::Marker,
            // Empty slots and editor-memory garbage are not level content.
            Mc1Kind::Empty | Mc1Kind::Junk => continue,
        };
        things.push(Thing {
            slot: slot as u32,
            kind,
            class: t.class,
            model: t.model,
            x: t.x,
            y: t.y,
            dis_id: t.dis_id,
            swi_sz: t.swi_sz,
            swi_id: t.swi_id,
            parent: t.parent,
            child: t.child,
            par3: None,
        });
    }

    let g = &level.gen_map;
    LevelPackage {
        meta: Meta {
            format_version: FORMAT_VERSION,
            game,
            level: level_index,
            source: Some(source),
            importer: importer(),
        },
        things: Things { things },
        header: None,
        wizards: None,
        stages: None,
        terrain: None,
        gen_params: Some(GenParams {
            pre_header: Some(g.pre_header),
            seed: g.seed,
            off: g.off,
            raise: g.raise,
            gnarl: g.gnarl,
            river: g.river,
            lriver: None,
            sourc: g.sourc,
            snlin: g.snlin,
            snflt: g.snflt,
            bhlin: g.bhlin,
            bhflt: g.bhflt,
            rkste: g.rkste,
            footer: Some(level.footer),
        }),
    }
}

/// Bake every level of one MC1-format archive into `out_dir/<tag>/`.
/// Returns `(package file name, sha256)` pairs.
pub fn bake_mc1_archive(
    game: Game,
    tag: &str,
    dat_path: &Path,
    tab_path: &Path,
    out_dir: &Path,
    genlevel: Option<&Path>,
) -> Result<Vec<(String, String)>, BakeError> {
    let read = |p: &Path| std::fs::read(p).map_err(|e| BakeError::Io(p.to_path_buf(), e));
    let archive = Archive::open(&read(dat_path)?, &read(tab_path)?)
        .map_err(|e| BakeError::Archive(dat_path.to_path_buf(), e))?;

    let archive_name = dat_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let game_dir = out_dir.join(tag);
    std::fs::create_dir_all(&game_dir).map_err(|e| BakeError::Io(game_dir.clone(), e))?;

    let mut outputs = Vec::new();
    for entry in archive.non_empty() {
        let raw = archive.raw(entry);
        let entry_sha256 = hex(&Sha256::digest(raw));

        let payload = archive.extract(entry).map_err(|e| {
            BakeError::Level(dat_path.to_path_buf(), entry.index as u32, e.to_string())
        })?;
        let level = Mc1Level::parse(&payload).map_err(|e| {
            BakeError::Level(dat_path.to_path_buf(), entry.index as u32, e.to_string())
        })?;

        let mut package = package_mc1_level(
            game,
            entry.index as u32,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: entry.index as u32,
                entry_sha256,
            },
        );
        if let Some(tool) = genlevel {
            package.terrain = Some(generate_terrain(
                tool,
                &mc1_oracle_payload(&level.gen_map),
                &game_dir,
                entry.index as u32,
            )?);
        }

        let name = format!("level-{:03}.mgcl", entry.index);
        let path = game_dir.join(&name);
        let file = std::fs::File::create(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        mgcl::write(std::io::BufWriter::new(file), &package)
            .map_err(|e| BakeError::Write(path.clone(), e))?;

        let baked = std::fs::read(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        outputs.push((format!("{tag}/{name}"), hex(&Sha256::digest(&baked))));
    }
    Ok(outputs)
}

/// Locate the terrain-generation oracle (`mc2-genlevel`, the original
/// algorithm carved out of remc2 — see tools/mc2-genlevel/): the
/// `MGC_GENLEVEL` environment variable, or the default in-repo build
/// location relative to the working directory.
pub fn find_genlevel() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MGC_GENLEVEL") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let default = Path::new("tools/mc2-genlevel/mc2-genlevel");
    default.exists().then(|| default.to_path_buf())
}

/// Bake MC1's environment palettes (`DATA/PAL0-0.DAT` day, `PAL1-0.DAT`
/// night; RNC-compressed 768-byte VGA palettes) into
/// `out_dir/mc1/assets/palette-{day,night}.bin` as 8-bit RGB. VGA DACs
/// are 6-bit; expansion replicates the top bits (`v<<2 | v>>4`) so full
/// white maps to 255 — the standard lossless-round-trip expansion.
pub fn bake_mc1_palettes(
    data_dir: &Path,
    out_dir: &Path,
) -> Result<Vec<(String, String)>, BakeError> {
    let assets_dir = out_dir.join("mc1/assets");
    std::fs::create_dir_all(&assets_dir).map_err(|e| BakeError::Io(assets_dir.clone(), e))?;

    let mut outputs = Vec::new();
    let mut emit = |name: &str, bytes: &[u8]| -> Result<(), BakeError> {
        let out_name = format!("{name}.bin");
        let path = assets_dir.join(&out_name);
        std::fs::write(&path, bytes).map_err(|e| BakeError::Io(path.clone(), e))?;
        outputs.push((
            format!("mc1/assets/{out_name}"),
            hex(&Sha256::digest(bytes)),
        ));
        Ok(())
    };
    let unpack = |file: &str| -> Result<Vec<u8>, BakeError> {
        let src = data_dir.join(file);
        let raw = std::fs::read(&src).map_err(|e| BakeError::Io(src.clone(), e))?;
        crate::rnc::decompress(&raw).map_err(|e| BakeError::Level(src.clone(), 0, e.to_string()))
    };

    for (file, name) in [
        ("PAL0-0.DAT", "palette-day"),
        ("PAL1-0.DAT", "palette-night"),
    ] {
        let vga = unpack(file)?;
        if vga.len() != 768 {
            return Err(BakeError::Level(
                data_dir.join(file),
                0,
                format!("palette is {} bytes, expected 768", vga.len()),
            ));
        }
        let rgb: Vec<u8> = vga.iter().map(|&v| (v << 2) | (v >> 4)).collect();
        emit(name, &rgb)?;
    }

    // Color-remap tables from the decompressed TABLES.DAT, exactly as
    // the engine's map view resolves tile colors (remc2 GameUI.cpp; MC2
    // splits the same layout into per-environment TABLESD/N/C.DAT):
    //   base  = tables[0x14000 + terrainType]      (tile-colors.bin)
    //   final = tables[shading * 256 + base]       (shade-lut.bin)
    //   rgb   = palette[final]
    let tables = unpack("TABLES.DAT")?;
    const SHADE_LUT_LEN: usize = 0x4000; // 64 shade levels x 256 colors
    const TILE_COLORS_OFFSET: usize = 0x14000;
    if tables.len() < TILE_COLORS_OFFSET + 256 {
        return Err(BakeError::Level(
            data_dir.join("TABLES.DAT"),
            0,
            format!("tables blob is {} bytes, expected >= 0x14100", tables.len()),
        ));
    }
    emit(
        "tile-colors",
        &tables[TILE_COLORS_OFFSET..TILE_COLORS_OFFSET + 256],
    )?;
    emit("shade-lut", &tables[..SHADE_LUT_LEN])?;
    Ok(outputs)
}

/// Synthesize a minimal MC2 level buffer carrying MC1 GEN_MAP params at
/// the offsets the oracle reads, so MC1 terrain can be generated by the
/// same tool. Validated by entity-placement coherence across MC1 levels
/// (see docs/ROADMAP.md "MC1 terrain oracle"): heights and water are
/// faithful; the tile-type snow/rock layers are not (MC1's snlin scale
/// exceeds MC2's) and need MC1-specific semantics later. MC1 has no
/// `lriver`; 0 keeps the generator's extra river pass inert.
fn mc1_oracle_payload(g: &crate::level_mc1::GenMap) -> Vec<u8> {
    let mut buf = vec![0u8; MC2_LEVEL_SIZE];
    let put16 = |buf: &mut [u8], o: usize, v: u16| buf[o..o + 2].copy_from_slice(&v.to_le_bytes());
    put16(&mut buf, 0x00, 2); // version the oracle accepts
    buf[0x06] = 0; // map type: day
    put16(&mut buf, 0x17, g.seed as u16);
    put16(&mut buf, 0x1B, g.off as u16);
    // Negative raise survives truncation: the generator reads __int16.
    put16(&mut buf, 0x1F, g.raise as u16);
    put16(&mut buf, 0x23, g.gnarl as u16);
    buf[0x27..0x2B].copy_from_slice(&g.river.to_le_bytes());
    put16(&mut buf, 0x2B, 0); // lriver: MC1 has none
    put16(&mut buf, 0x2F, g.sourc as u16);
    put16(&mut buf, 0x33, g.snlin as u16);
    put16(&mut buf, 0x37, g.snflt as u16);
    put16(&mut buf, 0x3B, g.bhlin as u16);
    put16(&mut buf, 0x3F, g.bhflt as u16);
    put16(&mut buf, 0x43, g.rkste as u16);
    buf
}

/// Run the oracle over one decompressed MC2 level, returning the
/// pristine generated terrain. The tool emits the engine's 0x70000
/// terrain block; we keep tile type (+0x00000) and heightmap (+0x10000).
fn generate_terrain(
    tool: &Path,
    payload: &[u8],
    scratch_dir: &Path,
    index: u32,
) -> Result<Terrain, BakeError> {
    let io_err = |e: std::io::Error| BakeError::Io(tool.to_path_buf(), e);
    let level_path = scratch_dir.join(format!("genlevel-in-{index}.bin"));
    let out_path = scratch_dir.join(format!("genlevel-out-{index}.bin"));
    std::fs::write(&level_path, payload).map_err(io_err)?;

    let status = std::process::Command::new(tool)
        .arg(&level_path)
        .arg(&out_path)
        .status()
        .map_err(io_err)?;
    if !status.success() {
        return Err(BakeError::Level(
            tool.to_path_buf(),
            index,
            format!("mc2-genlevel exited with {status}"),
        ));
    }
    let block = std::fs::read(&out_path).map_err(io_err)?;
    std::fs::remove_file(&level_path).ok();
    std::fs::remove_file(&out_path).ok();
    if block.len() != 0x70000 {
        return Err(BakeError::Level(
            tool.to_path_buf(),
            index,
            format!("oracle output {} bytes, expected 0x70000", block.len()),
        ));
    }
    Ok(Terrain {
        tile_type: block[..TERRAIN_GRID_BYTES].to_vec(),
        height: block[TERRAIN_GRID_BYTES..2 * TERRAIN_GRID_BYTES].to_vec(),
        shading: Some(block[2 * TERRAIN_GRID_BYTES..3 * TERRAIN_GRID_BYTES].to_vec()),
    })
}

/// Convert one parsed MC2 level into a package.
pub fn package_mc2_level(level_index: u32, level: &Mc2Level, source: Source) -> LevelPackage {
    let mut things = Vec::new();
    for (slot, t) in level.things.iter().enumerate() {
        if !t.is_active() {
            continue;
        }
        things.push(Thing {
            slot: slot as u32,
            // MC2 class 0 = Conditional Spawn, real content (FORMAT.md).
            kind: ThingKind::Entity,
            class: t.class,
            model: t.model,
            x: t.x,
            y: t.y,
            dis_id: t.dis_id as u16,
            swi_sz: t.word10,
            swi_id: t.stage_tag as u16,
            parent: t.par1,
            child: t.par2,
            par3: Some(t.par3),
        });
    }

    let h = &level.header;
    let g = &level.gen_map;
    LevelPackage {
        meta: Meta {
            format_version: FORMAT_VERSION,
            game: Game::MagicCarpet2,
            level: level_index,
            source: Some(source),
            importer: importer(),
        },
        things: Things { things },
        header: Some(LevelHeader {
            level_id: h.level_id,
            gfx_type: h.gfx_type,
            map_type: match h.map_type {
                Mc2MapType::Day => MapType::Day,
                Mc2MapType::Night => MapType::Night,
                Mc2MapType::Cave => MapType::Cave,
                // Rejected earlier in bake_mc2_archive.
                Mc2MapType::Unknown(_) => unreachable!("unknown map type"),
            },
            players: h.players,
            unk05: h.unk05,
            unk07: h.unk07,
            unk09: h.unk09,
        }),
        wizards: Some(Wizards {
            wizards: level
                .wizards
                .iter()
                .map(|w| WizardConfig {
                    aggression: w.aggression,
                    reflexes: w.reflexes,
                    perception: w.perception,
                    life: w.life,
                    starting_spells: w.starting_spells.to_vec(),
                    unknown_spells: w.unknown_spells.to_vec(),
                    blocked_spells: w.blocked_spells.to_vec(),
                })
                .collect(),
        }),
        terrain: None,
        stages: Some(Stages {
            checkpoints: level
                .checkpoints
                .iter()
                .filter(|c| c.is_used())
                .map(|c| StageCheckpoint {
                    index: c.index,
                    stage: c.stage,
                    x: c.x,
                    y: c.y,
                })
                .collect(),
            variables: level
                .stage_vars
                .iter()
                .filter(|v| v.is_used())
                .map(|v| StageVar {
                    index: v.index,
                    stage: v.stage,
                    x: v.x,
                    y: v.y,
                    data: v.data,
                })
                .collect(),
        }),
        gen_params: Some(GenParams {
            pre_header: None,
            seed: g.seed as u32,
            off: g.off as u32,
            raise: g.raise as i32,
            gnarl: g.gnarl as u32,
            river: g.river,
            lriver: Some(g.lriver as u32),
            sourc: g.sourc as u32,
            snlin: g.snlin as u32,
            snflt: g.snflt as u32,
            bhlin: g.bhlin as u32,
            bhflt: g.bhflt as u32,
            rkste: g.rkste as u32,
            footer: None,
        }),
    }
}

/// Bake every standard level of the MC2 archive into `out_dir/mc2/`.
/// The 18 "extended" dev-leftover entries (older format, ~39 KB) are
/// skipped and their indices returned separately.
#[allow(clippy::type_complexity)]
pub fn bake_mc2_archive(
    dat_path: &Path,
    tab_path: &Path,
    out_dir: &Path,
    genlevel: Option<&Path>,
) -> Result<(Vec<(String, String)>, Vec<u32>), BakeError> {
    let read = |p: &Path| std::fs::read(p).map_err(|e| BakeError::Io(p.to_path_buf(), e));
    let archive = Archive::open(&read(dat_path)?, &read(tab_path)?)
        .map_err(|e| BakeError::Archive(dat_path.to_path_buf(), e))?;

    let archive_name = dat_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let game_dir = out_dir.join("mc2");
    std::fs::create_dir_all(&game_dir).map_err(|e| BakeError::Io(game_dir.clone(), e))?;

    let mut outputs = Vec::new();
    let mut skipped = Vec::new();
    for entry in archive.non_empty() {
        let raw = archive.raw(entry);
        let entry_sha256 = hex(&Sha256::digest(raw));

        let payload = archive.extract(entry).map_err(|e| {
            BakeError::Level(dat_path.to_path_buf(), entry.index as u32, e.to_string())
        })?;
        if payload.len() != MC2_LEVEL_SIZE {
            skipped.push(entry.index as u32);
            continue;
        }
        let level = Mc2Level::parse(&payload).map_err(|e| {
            BakeError::Level(dat_path.to_path_buf(), entry.index as u32, e.to_string())
        })?;
        if matches!(level.header.map_type, Mc2MapType::Unknown(_)) {
            return Err(BakeError::Level(
                dat_path.to_path_buf(),
                entry.index as u32,
                "unknown map type".into(),
            ));
        }

        let mut package = package_mc2_level(
            entry.index as u32,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: entry.index as u32,
                entry_sha256,
            },
        );
        if let Some(tool) = genlevel {
            package.terrain = Some(generate_terrain(
                tool,
                &payload,
                &game_dir,
                entry.index as u32,
            )?);
        }

        let name = format!("level-{:03}.mgcl", entry.index);
        let path = game_dir.join(&name);
        let file = std::fs::File::create(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        mgcl::write(std::io::BufWriter::new(file), &package)
            .map_err(|e| BakeError::Write(path.clone(), e))?;

        let baked = std::fs::read(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        outputs.push((format!("mc2/{name}"), hex(&Sha256::digest(&baked))));
    }
    Ok((outputs, skipped))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
