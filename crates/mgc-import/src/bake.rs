//! Baking: original archives in, `.mgcl` packages out.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::dattab::Archive;
use crate::gamedata::GameSource;
use crate::level_mc1::{Mc1Level, ThingKind as Mc1Kind};
use crate::level_mc2::{MC2_LEVEL_SIZE, MapType as Mc2MapType, Mc2Level};
use crate::overlay::{Overlay, OverlayLevel};
use mgc_formats::{
    BAKE_EPOCH, FORMAT_VERSION, Game, GenParams, Importer, LevelHeader, LevelPackage, MapType,
    Meta, Source, StageCheckpoint, StageVar, Stages, TERRAIN_GRID_BYTES, Terrain, Thing, ThingKind,
    Things, WizardConfig, Wizards, mgcl,
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
            bake_epoch: BAKE_EPOCH,
            game,
            level: level_index,
            source: Some(source),
            overlay: None,
            importer: importer(),
        },
        things: Things { things },
        header: None,
        wizards: Some(Wizards {
            wizards: level
                .wizards
                .iter()
                .zip(level.castle_levels)
                .map(|(w, castle)| WizardConfig {
                    aggression: w.aggression as i16,
                    reflexes: None,
                    perception: None,
                    life: None,
                    starting_spells: w.pregrant.to_vec(),
                    starting_spell_levels: Vec::new(),
                    blocked_spells: Vec::new(),
                    accuracy: Some(w.accuracy as i16),
                    tempo: Some(w.tempo as i16),
                    castle_level: Some(castle),
                    allowed_spells: Some(w.allowed.to_vec()),
                })
                .collect(),
            player_count: Some(level.player_count),
            tail_38800: Some(level.tail_38800),
        }),
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

/// One archive's bake: the `(package file name, sha256)` manifest
/// pairs, plus the applied overlay substitutions as
/// `<member> <- <overlay file>` lines for the baked tree's `MODDED`
/// marker (docs/MODDING.md); empty on a pristine bake.
pub struct ArchiveBake {
    pub outputs: Vec<(String, String)>,
    pub overlaid: Vec<String>,
}

/// The payload for one archive member, and the `entry_sha256` to stamp:
/// the community replacement file when the overlay targets this index
/// (hashed over the overlay file itself), the retail entry otherwise
/// (hashed over the raw, still-compressed bytes).
fn member_payload(
    archive: &Archive,
    entry: crate::dattab::Entry,
    dat_path: &Path,
    replacement: Option<&OverlayLevel>,
) -> Result<(Vec<u8>, String), BakeError> {
    match replacement {
        Some(o) => {
            let bytes = std::fs::read(&o.path).map_err(|e| BakeError::Io(o.path.clone(), e))?;
            let sha = hex(&Sha256::digest(&bytes));
            Ok((bytes, sha))
        }
        None => {
            let raw = archive.raw(entry);
            let sha = hex(&Sha256::digest(raw));
            let payload = archive.extract(entry).map_err(|e| {
                BakeError::Level(dat_path.to_path_buf(), entry.index as u32, e.to_string())
            })?;
            Ok((payload, sha))
        }
    }
}

/// Overlay files that targeted no bakeable member (empty slots, MC2's
/// extended-format leftovers) are reported, never silently inert.
fn warn_unapplied(tag: &str, overlay: &[OverlayLevel], applied: &BTreeSet<u32>) {
    for o in overlay {
        if !applied.contains(&o.index) {
            eprintln!(
                "warning: overlay {} NOT applied — {tag} has no bakeable member {}",
                o.rel, o.index
            );
        }
    }
}

/// Bake every level of one MC1-format archive into `out_dir/<tag>/`,
/// substituting `overlay` payloads by member index. `base` is the
/// archive's canonical path without extension, e.g. `LEVELS/DDLEVELS`.
///
/// Terrain comes from the native MC1 generator port (`mc1_terrain`) —
/// unlike MC2, no external oracle tool is involved.
pub fn bake_mc1_archive(
    game: Game,
    tag: &str,
    src: &GameSource,
    base: &str,
    out_dir: &Path,
    overlay: &[OverlayLevel],
) -> Result<ArchiveBake, BakeError> {
    let dat_path = PathBuf::from(format!("{base}.DAT"));
    let read = |rel: String| {
        src.read(&rel)
            .map_err(|e| BakeError::Io(PathBuf::from(rel), e))
    };
    let archive = Archive::open(&read(format!("{base}.DAT"))?, &read(format!("{base}.TAB"))?)
        .map_err(|e| BakeError::Archive(dat_path.clone(), e))?;

    let archive_name = dat_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let game_dir = out_dir.join(tag);
    std::fs::create_dir_all(&game_dir).map_err(|e| BakeError::Io(game_dir.clone(), e))?;

    let by_index: BTreeMap<u32, &OverlayLevel> = overlay.iter().map(|o| (o.index, o)).collect();
    let mut applied = BTreeSet::new();

    let mut outputs = Vec::new();
    let mut overlaid = Vec::new();
    for entry in archive.non_empty() {
        let index = entry.index as u32;
        let replacement = by_index.get(&index).copied();
        let (payload, entry_sha256) = member_payload(&archive, entry, &dat_path, replacement)?;
        let level = Mc1Level::parse(&payload).map_err(|e| {
            // Parse errors blame whichever file supplied the bytes.
            let blamed = replacement.map_or_else(|| dat_path.clone(), |o| o.path.clone());
            BakeError::Level(blamed, index, e.to_string())
        })?;

        let mut package = package_mc1_level(
            game,
            index,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: index,
                entry_sha256,
            },
        );
        if let Some(o) = replacement {
            package.meta.overlay = Some(o.rel.clone());
            println!("{tag}: level {index:03} OVERLAY {}", o.rel);
            overlaid.push(format!("{tag}/level-{index:03}.mgcl <- {}", o.rel));
            applied.insert(index);
        }
        let generated =
            crate::mc1_terrain::generate(&level.gen_map, matches!(game, Game::HiddenWorlds));
        package.terrain = Some(Terrain {
            tile_type: generated.tile_type,
            height: generated.height,
            shading: Some(generated.shading),
            angle: Some(generated.angle),
            // MC1 has no cave levels / second heightmap.
            ceiling: None,
        });

        let name = format!("level-{index:03}.mgcl");
        let path = game_dir.join(&name);
        let file = std::fs::File::create(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        mgcl::write(std::io::BufWriter::new(file), &package)
            .map_err(|e| BakeError::Write(path.clone(), e))?;

        let baked = std::fs::read(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        outputs.push((format!("{tag}/{name}"), hex(&Sha256::digest(&baked))));
    }
    warn_unapplied(tag, overlay, &applied);
    Ok(ArchiveBake { outputs, overlaid })
}

// MC1 asset baking lives in `crate::bundle` (unified asset bundles).

/// Generate one MC2 level's terrain natively ([`crate::mc2_terrain`], a
/// byte-exact port of the original algorithm; the retired external
/// `mc2-genlevel` oracle lives in git history). The ceiling plane is
/// all-zero off cave levels (the generator's `sub_43D50` never writes
/// it), so an all-zero plane is dropped from the package.
fn native_mc2_terrain(level: &Mc2Level) -> Terrain {
    let t = crate::mc2_terrain::generate(level);
    debug_assert_eq!(t.ceiling.len(), TERRAIN_GRID_BYTES);
    let ceiling = t.ceiling.iter().any(|&b| b != 0).then_some(t.ceiling);
    Terrain {
        tile_type: t.tile_type,
        height: t.height,
        shading: Some(t.shading),
        angle: Some(t.angle),
        ceiling,
    }
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
            bake_epoch: BAKE_EPOCH,
            game: Game::MagicCarpet2,
            level: level_index,
            source: Some(source),
            overlay: None,
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
            basic_height: h.basic_height,
            unk07: h.unk07,
            number_of_players: h.unk09,
        }),
        wizards: Some(Wizards {
            wizards: level
                .wizards
                .iter()
                .map(|w| WizardConfig {
                    aggression: w.aggression,
                    reflexes: Some(w.reflexes),
                    perception: Some(w.perception),
                    life: Some(w.life),
                    starting_spells: w.starting_spells.to_vec(),
                    starting_spell_levels: w.unknown_spells.to_vec(),
                    blocked_spells: w.blocked_spells.to_vec(),
                    accuracy: None,
                    tempo: None,
                    castle_level: None,
                    allowed_spells: None,
                })
                .collect(),
            player_count: None,
            tail_38800: None,
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
            // ALL 11 slots, VERBATIM and SLOT-ALIGNED — never filter.
            // The sim's InitStageVars port aligns on slot index, and
            // byte0 packs FLAG BITS in its high nibble (0x80 = match-
            // by-subtype, 0x40 = watch-model), so a signed `>= 0`
            // is_used filter both drops flagged rows (e.g.
            // level-000's 0x82 goat-graze anchors) and compacts the
            // survivors, misaligning the indices.
            variables: level
                .stage_vars
                .iter()
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

/// Bake every standard level of the MC2 archive into `out_dir/mc2/`,
/// substituting `overlay` payloads by member index. The 18 "extended"
/// dev-leftover entries (older format, ~39 KB) are skipped and their
/// indices returned separately — the skip is decided on the RETAIL
/// member, so those slots cannot be overlay targets.
pub fn bake_mc2_archive(
    src: &GameSource,
    out_dir: &Path,
    overlay: &[OverlayLevel],
) -> Result<(ArchiveBake, Vec<u32>), BakeError> {
    let dat_path = PathBuf::from("LEVELS/LEVELS.DAT");
    let read = |rel: &str| {
        src.read(rel)
            .map_err(|e| BakeError::Io(PathBuf::from(rel), e))
    };
    let archive = Archive::open(&read("LEVELS/LEVELS.DAT")?, &read("LEVELS/LEVELS.TAB")?)
        .map_err(|e| BakeError::Archive(dat_path.clone(), e))?;

    let archive_name = dat_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let game_dir = out_dir.join("mc2");
    std::fs::create_dir_all(&game_dir).map_err(|e| BakeError::Io(game_dir.clone(), e))?;

    let by_index: BTreeMap<u32, &OverlayLevel> = overlay.iter().map(|o| (o.index, o)).collect();
    let mut applied = BTreeSet::new();

    let mut outputs = Vec::new();
    let mut overlaid = Vec::new();
    let mut skipped = Vec::new();
    for entry in archive.non_empty() {
        let index = entry.index as u32;
        let raw = archive.raw(entry);
        let mut entry_sha256 = hex(&Sha256::digest(raw));

        let mut payload = archive
            .extract(entry)
            .map_err(|e| BakeError::Level(dat_path.to_path_buf(), index, e.to_string()))?;
        if payload.len() != MC2_LEVEL_SIZE {
            skipped.push(index);
            continue;
        }
        let replacement = by_index.get(&index).copied();
        if let Some(o) = replacement {
            payload = std::fs::read(&o.path).map_err(|e| BakeError::Io(o.path.clone(), e))?;
            if payload.len() != MC2_LEVEL_SIZE {
                return Err(BakeError::Level(
                    o.path.clone(),
                    index,
                    format!(
                        "MC2 level must be {MC2_LEVEL_SIZE} bytes, got {}",
                        payload.len()
                    ),
                ));
            }
            entry_sha256 = hex(&Sha256::digest(&payload));
        }
        let level = Mc2Level::parse(&payload).map_err(|e| {
            // Parse errors blame whichever file supplied the bytes.
            let blamed = replacement.map_or_else(|| dat_path.clone(), |o| o.path.clone());
            BakeError::Level(blamed, index, e.to_string())
        })?;
        if matches!(level.header.map_type, Mc2MapType::Unknown(_)) {
            let blamed = replacement.map_or_else(|| dat_path.clone(), |o| o.path.clone());
            return Err(BakeError::Level(blamed, index, "unknown map type".into()));
        }

        let mut package = package_mc2_level(
            index,
            &level,
            Source {
                archive: archive_name.clone(),
                entry: index,
                entry_sha256,
            },
        );
        if let Some(o) = replacement {
            package.meta.overlay = Some(o.rel.clone());
            println!("mc2: level {index:03} OVERLAY {}", o.rel);
            overlaid.push(format!("mc2/level-{index:03}.mgcl <- {}", o.rel));
            applied.insert(index);
        }
        package.terrain = Some(native_mc2_terrain(&level));

        let name = format!("level-{index:03}.mgcl");
        let path = game_dir.join(&name);
        let file = std::fs::File::create(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        mgcl::write(std::io::BufWriter::new(file), &package)
            .map_err(|e| BakeError::Write(path.clone(), e))?;

        let baked = std::fs::read(&path).map_err(|e| BakeError::Io(path.clone(), e))?;
        outputs.push((format!("mc2/{name}"), hex(&Sha256::digest(&baked))));
    }
    warn_unapplied("mc2", overlay, &applied);
    Ok((ArchiveBake { outputs, overlaid }, skipped))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Outcome of [`bake_all`]: the `(member, sha256)` pairs written to
/// `manifest.sha256`, sorted. Empty when no game data was found (a
/// valid outcome for the CLI; the game shell treats it as fatal).
pub struct BakeSummary {
    pub manifest: Vec<(String, String)>,
}

/// Bake every game found under `gamedata` into `out_dir` — the full
/// tree the engine consumes: level packages, environment bundles,
/// audio/music bundles, plus `manifest.sha256`. Any subset of the
/// three games is valid, including none at all (each absent source is
/// skipped with a note). This is the one orchestration path, shared by
/// the `mgc-import bake` CLI and the game shell's first-run/stale-epoch
/// auto-bake; progress and skip notes print to stdout/stderr in both.
pub fn bake_all(gamedata: &Path, out_dir: &Path) -> Result<BakeSummary, String> {
    let found = crate::gamedata::Gamedata::locate(gamedata);
    match &found.mc1 {
        Some(src) => println!("mc1 source: {}", src.origin),
        None => eprintln!("note: no MC1 data under {} — skipping", gamedata.display()),
    }
    match &found.mc2 {
        Some(src) => println!("mc2 source: {}", src.origin),
        None => eprintln!("note: no MC2 data under {} — skipping", gamedata.display()),
    }

    // MC1 and MC2 terrain are both generated natively (mc1_terrain /
    // mc2_terrain) — no external tool required.

    // The community/mod overlay, when present (docs/MODDING.md). NOT
    // epoch-tracked: overlay changes require deleting baked/.
    let overlay = Overlay::locate(gamedata);
    if let Some(ov) = &overlay {
        println!("overlay: {}", ov.root().display());
    }
    let overlay_levels = |tag: &str| overlay.as_ref().map_or(Ok(Vec::new()), |ov| ov.levels(tag));

    let mut manifest = Vec::new();
    let mut modded = Vec::new();
    if let Some(src) = &found.mc1 {
        let archives = [
            (Game::MagicCarpet1, "mc1", "LEVELS/LEVELS"),
            (Game::HiddenWorlds, "mc1hw", "LEVELS/DDLEVELS"),
        ];
        for (game, tag, base) in archives {
            if !src.exists(&format!("{base}.DAT")) {
                eprintln!("note: {base}.DAT not found — skipping {tag}");
                continue;
            }
            let bake = bake_mc1_archive(game, tag, src, base, out_dir, &overlay_levels(tag)?)
                .map_err(|e| e.to_string())?;
            println!("{tag}: baked {} levels", bake.outputs.len());
            manifest.extend(bake.outputs);
            modded.extend(bake.overlaid);
        }
        if src.exists("DATA/PAL0-0.DAT") {
            let outputs =
                crate::bundle::bake_mc1_bundles(src, out_dir).map_err(|e| e.to_string())?;
            println!(
                "mc1: baked asset bundles mc1-temperate + mc1-arctic ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        } else {
            eprintln!("note: mc1 DATA/PAL0-0.DAT not found — skipping asset bundles");
        }
        if src.exists("DATA/SNDS0-1.DAT") {
            let outputs = crate::bundle::bake_mc1_audio(src, out_dir).map_err(|e| e.to_string())?;
            println!(
                "mc1: baked audio bundle mc1-audio ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        } else {
            eprintln!("note: mc1 DATA/SNDS0-1.DAT not found — skipping audio bundle");
        }
        let outputs = crate::bundle::bake_mc1_menu(src, out_dir).map_err(|e| e.to_string())?;
        if !outputs.is_empty() {
            println!(
                "mc1: baked frontend bundle mc1-ui ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        }
        let outputs =
            crate::bundle::bake_movies(src, out_dir, "mc1-movies", mgc_formats::Game::MagicCarpet1)
                .map_err(|e| e.to_string())?;
        if !outputs.is_empty() {
            println!(
                "mc1: baked movie bundle mc1-movies ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        }
    }

    if let Some(src) = &found.mc2 {
        let (bake, skipped) =
            bake_mc2_archive(src, out_dir, &overlay_levels("mc2")?).map_err(|e| e.to_string())?;
        println!("mc2: baked {} levels", bake.outputs.len());
        if !skipped.is_empty() {
            println!(
                "mc2: skipped {} extended-format dev leftovers (indices {:?})",
                skipped.len(),
                skipped
            );
        }
        manifest.extend(bake.outputs);
        modded.extend(bake.overlaid);
        // Environment bundles need the CD catalogs (absent from
        // hard-disk-only legacy copies).
        if src.exists("DATA/PALD-0.DAT") {
            let outputs =
                crate::bundle::bake_mc2_bundles(src, out_dir).map_err(|e| e.to_string())?;
            println!(
                "mc2: baked asset bundles mc2-day/night/night-fog/cave ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        } else {
            eprintln!(
                "note: mc2 DATA/PALD-0.DAT not found (CD catalogs missing) — skipping mc2 bundles"
            );
        }
        let outputs = crate::bundle::bake_mc2_audio(src, out_dir).map_err(|e| e.to_string())?;
        if !outputs.is_empty() {
            println!(
                "mc2: baked audio bundle mc2-audio ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        }
        let outputs = crate::bundle::bake_mc2_worldmap(src, out_dir).map_err(|e| e.to_string())?;
        if !outputs.is_empty() {
            println!(
                "mc2: baked world-map bundle mc2-ui ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        }
        let outputs =
            crate::bundle::bake_movies(src, out_dir, "mc2-movies", mgc_formats::Game::MagicCarpet2)
                .map_err(|e| e.to_string())?;
        if !outputs.is_empty() {
            println!(
                "mc2: baked movie bundle mc2-movies ({} members)",
                outputs.len()
            );
            manifest.extend(outputs);
        }
    }

    // The tree-level pristine/modded discriminator: a bake that
    // applied ANY overlay file writes the MODDED marker listing every
    // substitution; a pristine bake removes it. Goldens and
    // conformance key off it (docs/MODDING.md).
    let modded_path = out_dir.join("MODDED");
    if modded.is_empty() {
        let _ = std::fs::remove_file(&modded_path);
    } else {
        modded.sort();
        let body = format!(
            "# MODDED bake: community-overlay files were applied (docs/MODDING.md).\n\
             # Not a faithful fixture — goldens and conformance refuse this tree.\n\
             # For a pristine tree, delete baked/ and rebake without gamedata/overlay/.\n{}",
            modded.iter().map(|l| format!("{l}\n")).collect::<String>()
        );
        std::fs::write(&modded_path, body)
            .map_err(|e| format!("cannot write {}: {e}", modded_path.display()))?;
        println!(
            "MODDED bake: {} overlay file(s) applied — not a faithful fixture ({})",
            modded.len(),
            modded_path.display()
        );
    }

    if manifest.is_empty() {
        return Ok(BakeSummary { manifest });
    }

    manifest.sort();
    let manifest_path = out_dir.join("manifest.sha256");
    let body: String = manifest
        .iter()
        .map(|(name, hash)| format!("{hash}  {name}\n"))
        .collect();
    std::fs::write(&manifest_path, body)
        .map_err(|e| format!("cannot write {}: {e}", manifest_path.display()))?;
    println!(
        "{} packages, manifest: {}",
        manifest.len(),
        manifest_path.display()
    );
    Ok(BakeSummary { manifest })
}
