//! Integration test: parse every standard MC2 level from real game data
//! and check the spec's invariants. Self-skips when game data is absent.
//!
//! Strong cross-checks from michaelhoward's MC2 spec:
//! - 165 standard (26,116-byte) levels;
//! - environment census: 77 Day, 41 Night, 47 Cave;
//! - the hidden realm at index 28 is a Cave level;
//! - stage tables are 0xFF-filled after the last used entry.

use std::path::{Path, PathBuf};

use mgc_import::dattab::Archive;
use mgc_import::level_mc2::{MC2_LEVEL_SIZE, MapType, Mc2Level};

fn gamedata_dir() -> PathBuf {
    match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    }
}

#[test]
fn mc2_levels_parse_and_hold_invariants() {
    let root = gamedata_dir().join("mc2/GAME/NETHERW/CLEVELS");
    let (dat, tab) = (root.join("LEVELS.DAT"), root.join("LEVELS.TAB"));
    if !dat.exists() {
        eprintln!("note: MC2 level data not present — skipping");
        return;
    }
    let archive =
        Archive::open(&std::fs::read(&dat).unwrap(), &std::fs::read(&tab).unwrap()).unwrap();

    let mut standard = 0u32;
    let mut extended = 0u32;
    let mut census = std::collections::BTreeMap::<&str, u32>::new();
    let mut cave_at_28 = false;
    let mut empty_levels = Vec::new();

    for entry in archive.non_empty() {
        let payload = archive.extract(entry).unwrap();
        if payload.len() != MC2_LEVEL_SIZE {
            extended += 1;
            continue;
        }
        standard += 1;
        let level =
            Mc2Level::parse(&payload).unwrap_or_else(|e| panic!("MC2 level {}: {e}", entry.index));

        let tag = match level.header.map_type {
            MapType::Day => "day",
            MapType::Night => "night",
            MapType::Cave => "cave",
            MapType::Unknown(v) => panic!("MC2 level {}: map type {v}", entry.index),
        };
        *census.entry(tag).or_default() += 1;
        // The hidden realm "at index 28" lives in the shared TAB slot
        // group 28-30 (aliased zero-length entries); the bytes land on
        // the group's last slot in delta terms.
        if (28..=30).contains(&entry.index) && level.header.map_type == MapType::Cave {
            cave_at_28 = true;
        }

        assert!(
            !level.reserved_nonzero,
            "MC2 level {}: reserved block not zeros",
            entry.index
        );

        // Coordinate sanity doubles as an endianness check: a byte-order
        // mistake turns x=100 into 25600 instantly. Terrain-only dev
        // testbeds (e.g. index 69: empty entity table, valid seed) exist
        // in the archive's non-campaign region; campaign levels must
        // have entities.
        let active = level.active_things().count();
        if entry.index < 25 {
            assert!(
                active > 0,
                "MC2 campaign level {} has no entities",
                entry.index
            );
        } else if active == 0 {
            empty_levels.push(entry.index);
        }
        for (slot, thing) in level.active_things() {
            assert!(
                thing.x < 256 && thing.y < 256,
                "MC2 level {} slot {slot}: position ({}, {}) off-grid",
                entry.index,
                thing.x,
                thing.y,
            );
        }

        for (i, w) in level.wizards.iter().enumerate() {
            // Inactive wizard slots carry uninitialized data (e.g. level
            // 67 slot 6: aggression 0x7878) — only active slots must obey
            // the documented ranges.
            if level.header.players[i] == 0 {
                continue;
            }
            assert!(
                (0..=255).contains(&w.aggression)
                    && (0..=255).contains(&w.reflexes)
                    && (0..=255).contains(&w.perception),
                "MC2 level {} wizard {i}: AI stats out of range ({}, {}, {})",
                entry.index,
                w.aggression,
                w.reflexes,
                w.perception,
            );
            assert!(
                w.starting_spells.iter().all(|&t| t <= 3),
                "MC2 level {} wizard {i}: spell tier > 3",
                entry.index,
            );
        }

        // Used stage entries precede unused ones — campaign discipline
        // only; dev levels (e.g. index 88) have sparse tables.
        if entry.index >= 25 {
            continue;
        }
        let first_unused = level
            .checkpoints
            .iter()
            .position(|c| !c.is_used())
            .unwrap_or(level.checkpoints.len());
        assert!(
            level.checkpoints[first_unused..]
                .iter()
                .all(|c| !c.is_used()),
            "MC2 level {}: used checkpoint after ff-fill",
            entry.index
        );
    }

    eprintln!(
        "MC2: {standard} standard levels parsed, {extended} extended skipped, census {census:?}, terrain-only: {empty_levels:?}"
    );
    assert_eq!(standard, 165);
    assert_eq!(extended, 18);
    assert_eq!(census.get("day"), Some(&77));
    assert_eq!(census.get("night"), Some(&41));
    assert_eq!(census.get("cave"), Some(&47));
    assert!(
        cave_at_28,
        "hidden realm in slot group 28-30 should be a cave level"
    );
}
