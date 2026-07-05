//! Integration test: parse every MC1 and Hidden Worlds level from real
//! game data and check the spec's invariants hold across all of them.
//! Self-skips when game data is absent.

use std::path::{Path, PathBuf};

use mgc_import::dattab::Archive;
use mgc_import::gamedata::{GameSource, Gamedata};
use mgc_import::level_mc1::Mc1Level;

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

fn check_all(archive: &Archive, label: &str) -> Vec<Mc1Level> {
    let mut levels = Vec::new();
    for entry in archive.non_empty() {
        let payload = archive.extract(entry).unwrap();
        let level = Mc1Level::parse(&payload)
            .unwrap_or_else(|e| panic!("{label} level {}: {e}", entry.index));

        // Dev leftovers (e.g. DDLEVELS index 198) can be terrain-only
        // with zero placed entities but still carry markers.
        let active = level.active_things().count();
        assert!(
            active > 0 || level.markers().count() > 0,
            "{label} level {} has no content at all",
            entry.index
        );

        // Markers share the entity grid; junk is unconstrained.
        for (slot, marker) in level.markers() {
            assert!(
                marker.x < 256 && marker.y < 256,
                "{label} level {} marker slot {slot} off-grid",
                entry.index,
            );
        }

        for (slot, thing) in level.active_things() {
            // Map is a 256x256 logical grid.
            assert!(
                thing.x < 256 && thing.y < 256,
                "{label} level {} slot {slot}: position ({}, {}) outside 256x256 grid",
                entry.index,
                thing.x,
                thing.y,
            );
        }

        // Loose sanity bound on terrain parameters — catches struct
        // misalignment, not gameplay ranges (the spec's gnarl<=128 was
        // campaign-only; Hidden Worlds ships gnarl up to ~200).
        let g = &level.gen_map;
        assert!(
            g.gnarl < 4096,
            "{label} level {}: gnarl {} (misaligned parse?)",
            entry.index,
            g.gnarl
        );

        levels.push(level);
    }
    levels
}

#[test]
fn mc1_levels_parse_and_hold_invariants() {
    let Some(archive) = gamedata().mc1.and_then(|s| open(&s, "LEVELS/LEVELS")) else {
        eprintln!("note: MC1 level data not present — skipping");
        return;
    };
    let levels = check_all(&archive, "MC1");
    assert_eq!(levels.len(), 70);

    // The spec's cross-check: level 0 has exactly 545 active entities.
    assert_eq!(
        levels[0].active_things().count(),
        545,
        "MC1 level 0 active-entity count diverges from spec"
    );
    // Reserved block is all zeros across the base campaign.
    assert!(
        levels.iter().all(|l| !l.reserved_nonzero),
        "MC1: reserved block unexpectedly non-zero"
    );

    let rivers = levels.iter().filter(|l| l.gen_map.river > 0).count();
    eprintln!("MC1: 70 levels parsed, {rivers} with rivers");
}

#[test]
fn hidden_worlds_levels_parse_and_hold_invariants() {
    let Some(archive) = gamedata().mc1.and_then(|s| open(&s, "LEVELS/DDLEVELS")) else {
        eprintln!("note: Hidden Worlds level data not present — skipping");
        return;
    };
    let levels = check_all(&archive, "DDLEVELS");
    eprintln!("DDLEVELS: {} levels parsed", levels.len());
    let nonzero_reserved = levels.iter().filter(|l| l.reserved_nonzero).count();
    if nonzero_reserved > 0 {
        eprintln!("DDLEVELS: {nonzero_reserved} levels have non-zero reserved blocks");
    }
}
