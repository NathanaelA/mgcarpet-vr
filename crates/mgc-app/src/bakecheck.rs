//! First-run / stale-bake detection. The game looks for its baked
//! data and (re)bakes it from the original game data when it is
//! missing or from an older bake (`mgc_formats::BAKE_EPOCH`), so a
//! fresh checkout + GOG installs is all a player needs: point the
//! game at a level and the importer runs by itself, once.
//!
//! Staleness is judged from the artifact stamps, not timestamps: the
//! requested level package's `meta.json` and every present bundle's
//! `bundle.json` must carry the current schema version AND the
//! current bake epoch. Any mismatch (or a missing artifact) triggers
//! one full `bake_all` — the same orchestration the `mgc-import bake`
//! CLI runs — into the baked tree, from game data located via (in
//! order) the config's `gamedata` path, `MGC_GAMEDATA`, or
//! `gamedata/` in the working directory.

use std::path::{Path, PathBuf};

use mgc_formats::bundle::BUNDLE_VERSION;
use mgc_formats::{BAKE_EPOCH, FORMAT_VERSION, mgcl};

/// Why the baked tree needs regenerating; `None` = current.
fn level_staleness(level_path: &Path) -> Option<String> {
    let file = match std::fs::File::open(level_path) {
        Ok(f) => f,
        Err(_) => return Some(format!("{} is not baked yet", level_path.display())),
    };
    let meta = match mgcl::read_meta(file) {
        Ok(m) => m,
        Err(e) => return Some(format!("{}: unreadable ({e})", level_path.display())),
    };
    if meta.format_version != FORMAT_VERSION || meta.bake_epoch != BAKE_EPOCH {
        return Some(format!(
            "{}: baked as format {} epoch {}, this build wants {}/{}",
            level_path.display(),
            meta.format_version,
            meta.bake_epoch,
            FORMAT_VERSION,
            BAKE_EPOCH,
        ));
    }
    None
}

/// Sweep `<baked>/assets/*/bundle.json`. A stale stamp anywhere (env,
/// audio) triggers the rebake; an EMPTY assets tree does too — no
/// level can load without at least one environment bundle.
fn bundle_staleness(baked_root: &Path) -> Option<String> {
    let assets = baked_root.join("assets");
    let entries = match std::fs::read_dir(&assets) {
        Ok(e) => e,
        Err(_) => return Some(format!("{}: no asset bundles baked", assets.display())),
    };
    let mut bundles = 0usize;
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("bundle.json");
        let bytes = match std::fs::read(&manifest_path) {
            Ok(b) => b,
            Err(_) => continue, // not a bundle dir
        };
        bundles += 1;
        let manifest: mgc_formats::bundle::BundleManifest = match serde_json::from_slice(&bytes) {
            Ok(m) => m,
            Err(e) => {
                return Some(format!("{}: unreadable ({e})", manifest_path.display()));
            }
        };
        if manifest.format_version != BUNDLE_VERSION || manifest.bake_epoch != BAKE_EPOCH {
            return Some(format!(
                "bundle {}: baked as format {} epoch {}, this build wants {}/{}",
                manifest.variant,
                manifest.format_version,
                manifest.bake_epoch,
                BUNDLE_VERSION,
                BAKE_EPOCH,
            ));
        }
    }
    (bundles == 0).then(|| format!("{}: no asset bundles baked", assets.display()))
}

/// Locate original game data: config override, `MGC_GAMEDATA`, then
/// `gamedata/` beside the working directory.
fn locate_gamedata(config_override: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = config_override {
        return p.is_dir().then(|| p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("MGC_GAMEDATA") {
        let p = PathBuf::from(p);
        return p.is_dir().then_some(p);
    }
    let default = Path::new("gamedata");
    default.is_dir().then(|| default.to_path_buf())
}

/// Ensure the baked tree serving `level_path` is present and current;
/// bake it from game data when it isn't. `Ok(())` means the caller
/// can load the level normally.
pub fn ensure_baked(level_path: &Path, config_gamedata: Option<&Path>) -> Result<(), String> {
    // Same root rule as load_level: <baked>/<game>/level-NNN.mgcl —
    // but only trust the inference when the path actually follows the
    // convention (parent dir named after a game). Guessing a root for
    // an arbitrary path can spray a full bake into the working
    // directory (a bare `mc3:5` would infer root `.`).
    let parent_is_game = level_path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|d| matches!(d.to_str(), Some("mc1" | "mc1hw" | "mc2")));
    if !parent_is_game {
        // A custom/one-off package outside the baked tree: load it as
        // it stands (bundle resolution reports its own errors), but
        // never auto-bake into an inferred root.
        return if level_path.exists() {
            Ok(())
        } else {
            Err(format!(
                "{}: not found, and the path does not follow \
                 <root>/<game>/level-NNN.mgcl, so there is no baked tree to \
                 (re)generate for it. Try `--level mc1:0` or a path under \
                 baked/.",
                level_path.display()
            ))
        };
    }
    let baked_root = level_path
        .parent()
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));

    let reason = level_staleness(level_path).or_else(|| bundle_staleness(baked_root));
    let Some(reason) = reason else {
        return Ok(());
    };

    let Some(gamedata) = locate_gamedata(config_gamedata) else {
        return Err(format!(
            "baked data needs (re)generating ({reason}), but no game data was found.\n\
             Point the game at your original GOG install(s): set `gamedata` in the\n\
             config file, set MGC_GAMEDATA, or place them under gamedata/ (see\n\
             gamedata/README.md for the expected layout)."
        ));
    };

    println!("baking game data ({reason})");
    println!(
        "  {} -> {}  (first run does everything, including music rendering — this can take a while)",
        gamedata.display(),
        baked_root.display()
    );
    let summary = mgc_import::bake::bake_all(&gamedata, baked_root)?;
    if summary.manifest.is_empty() {
        return Err(format!(
            "no game data found under {} — nothing baked. Point `gamedata` in the\n\
             config or MGC_GAMEDATA at your original GOG install(s).",
            gamedata.display()
        ));
    }

    // The bake ran; verify it actually produced what we came for
    // (e.g. the requested level might belong to a game whose data is
    // absent).
    if let Some(still) = level_staleness(level_path).or_else(|| bundle_staleness(baked_root)) {
        return Err(format!(
            "bake completed but the requested data is still unavailable: {still}"
        ));
    }
    Ok(())
}
