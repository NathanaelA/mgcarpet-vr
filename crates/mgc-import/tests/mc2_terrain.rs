//! Golden regression guard for the native MC2 terrain generator
//! ([`mgc_import::mc2_terrain`]). Generates every real MC2 level's
//! terrain and pins a single SHA-256 over all five planes of all levels.
//!
//! This replaces the one-time live cross-check against the external
//! `mc2-genlevel` oracle, which certified the port byte-for-byte and was
//! then retired (the C++ lives in git history). The generator is a port
//! of a frozen algorithm, so a moved hash means a real regression — or a
//! DELIBERATE change, in which case re-pin `GOLDEN` from the value the
//! test prints (run with `--nocapture`).
//!
//! Self-skips without gamedata; needs no external tool.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use mgc_import::dattab::Archive;
use mgc_import::gamedata::Gamedata;
use mgc_import::level_mc2::{MapType, Mc2Level};
use mgc_import::mc2_terrain;

const MC2_LEVEL_SIZE: usize = 26116;

/// SHA-256 over `(index_le32 ‖ type ‖ height ‖ shading ‖ angle ‖
/// ceiling)` for every baked MC2 level, in archive order. Certified
/// equal to the `mc2-genlevel` oracle across all 165 levels × 5 planes
/// at the time of pinning.
const GOLDEN: &str = "3b8cf786c8331f3bcd3b8c3d4200e710a325dd73a35e3b5ba97402af6aa0c7f0";

fn gamedata() -> Gamedata {
    let root = match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    };
    Gamedata::locate(&root)
}

#[test]
fn native_mc2_terrain_golden() {
    let Some(src) = gamedata().mc2 else {
        eprintln!("note: MC2 level data not present — skipping");
        return;
    };

    let archive = Archive::open(
        &src.read("LEVELS/LEVELS.DAT").unwrap(),
        &src.read("LEVELS/LEVELS.TAB").unwrap(),
    )
    .unwrap();

    let mut hasher = Sha256::new();
    let mut checked = 0u32;
    for entry in archive.non_empty() {
        let payload = archive.extract(entry).unwrap();
        if payload.len() != MC2_LEVEL_SIZE {
            continue; // extended dev-leftover entries — bake skips these too
        }
        let level = Mc2Level::parse(&payload).unwrap();
        if matches!(level.header.map_type, MapType::Unknown(_)) {
            continue;
        }
        let t = mc2_terrain::generate(&level);
        hasher.update((entry.index as u32).to_le_bytes());
        hasher.update(&t.tile_type);
        hasher.update(&t.height);
        hasher.update(&t.shading);
        hasher.update(&t.angle);
        hasher.update(&t.ceiling);
        checked += 1;
    }

    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    eprintln!("mc2 terrain golden: {checked} levels, hash {got}");
    assert_eq!(
        got, GOLDEN,
        "native MC2 terrain diverged from its golden — if DELIBERATE, re-pin GOLDEN to the printed hash"
    );
}
