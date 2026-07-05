//! Integration test: open both games' level archives and extract every
//! level. Self-skips when game data is absent (see tests/gamedata.rs).
//!
//! Facts asserted here come from the reference implementations: remc2
//! reads MC2 levels as `tab[i]..tab[i+1]` slices of CLEVELS/LEVELS.DAT,
//! each an RNC container holding a 0x6604-byte compressed-level struct.

use std::path::{Path, PathBuf};

use mgc_import::dattab::Archive;
use mgc_import::gamedata::{GameSource, Gamedata};

fn gamedata() -> Gamedata {
    let root = match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    };
    Gamedata::locate(&root)
}

fn open(src: &GameSource, base: &str) -> Option<Archive> {
    let dat = src.read(&format!("{base}.DAT")).ok()?;
    let tab = src.read(&format!("{base}.TAB")).ok()?;
    Some(Archive::open(&dat, &tab).expect("archive should parse"))
}

fn extract_all(archive: &Archive, label: &str) -> usize {
    let mut count = 0;
    for entry in archive.non_empty() {
        let payload = archive
            .extract(entry)
            .unwrap_or_else(|e| panic!("{label} entry {}: {e}", entry.index));
        assert!(!payload.is_empty(), "{label} entry {} empty", entry.index);
        count += 1;
    }
    count
}

#[test]
fn mc1_levels_extract() {
    let Some(archive) = gamedata().mc1.and_then(|s| open(&s, "LEVELS/LEVELS")) else {
        eprintln!("note: MC1 level data not present — skipping");
        return;
    };
    let count = extract_all(&archive, "MC1 LEVELS");
    eprintln!("MC1 LEVELS: {count} levels extracted");
    // The 1994 campaign ships 50 levels.
    assert!(count >= 50, "expected at least 50 MC1 levels, got {count}");
}

#[test]
fn mc1_hidden_worlds_levels_extract() {
    let Some(archive) = gamedata().mc1.and_then(|s| open(&s, "LEVELS/DDLEVELS")) else {
        eprintln!("note: Hidden Worlds level data not present — skipping");
        return;
    };
    let count = extract_all(&archive, "MC1 DDLEVELS");
    eprintln!("MC1 DDLEVELS (Hidden Worlds): {count} levels extracted");
    assert!(count > 0);
}

#[test]
fn mc2_levels_extract() {
    let Some(archive) = gamedata().mc2.and_then(|s| open(&s, "LEVELS/LEVELS")) else {
        eprintln!("note: MC2 level data not present — skipping");
        return;
    };
    let mut sizes = std::collections::BTreeMap::<usize, u32>::new();
    let mut count = 0;
    for entry in archive.non_empty() {
        let payload = archive
            .extract(entry)
            .unwrap_or_else(|e| panic!("MC2 LEVELS entry {}: {e}", entry.index));
        *sizes.entry(payload.len()).or_default() += 1;
        count += 1;
        // remc2 copies each campaign level into a 0x6604-byte struct;
        // higher indices include unused dev leftovers in older formats
        // (~39 KB, near MC1's 38812) that the game never loads.
        if entry.index < 25 {
            assert_eq!(
                payload.len(),
                0x6604,
                "MC2 campaign level {} has unexpected size",
                entry.index
            );
        }
    }
    eprintln!("MC2 LEVELS: {count} levels extracted, payload sizes: {sizes:?}");
    // The campaign is 25 levels but the dev archive ships many more
    // (unused levels documented on TCRF).
    assert!(count >= 25, "expected at least 25 MC2 levels, got {count}");
}
