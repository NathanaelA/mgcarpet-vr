//! Baking: original archives in, `.mgcl` packages out.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::dattab::Archive;
use crate::level_mc1::{Mc1Level, ThingKind as Mc1Kind};
use crate::level_mc2::{MC2_LEVEL_SIZE, MapType as Mc2MapType, Mc2Level};
use mgc_formats::{
    FORMAT_VERSION, Game, GenParams, Importer, LevelHeader, LevelPackage, MapType, Meta, Source,
    StageCheckpoint, StageVar, Stages, Thing, ThingKind, Things, WizardConfig, Wizards, mgcl,
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

        let package = package_mc1_level(
            game,
            entry.index as u32,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: entry.index as u32,
                entry_sha256,
            },
        );

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

        let package = package_mc2_level(
            entry.index as u32,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: entry.index as u32,
                entry_sha256,
            },
        );

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
