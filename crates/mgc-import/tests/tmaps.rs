//! Integration test: parse every retail TMAPS archive and decompress
//! every entry, verifying the wide-TAB layout holds across both games.
//! Self-skips without game data.

use std::path::{Path, PathBuf};

use mgc_import::tmaps::TmapsArchive;

fn gamedata_dir() -> PathBuf {
    match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    }
}

#[test]
fn retail_tmaps_archives_fully_extract() {
    let sets: [(&str, usize); 5] = [
        // (path base relative to gamedata, expected entry count)
        ("mc1/DATA/TMAPS0-0", 529),
        ("mc1/DATA/TMAPS1-0", 529),
        ("mc2/GAME/NETHERW/CDATA/TMAPS0-0", 504),
        ("mc2/GAME/NETHERW/CDATA/TMAPS1-0", 504),
        ("mc2/GAME/NETHERW/CDATA/TMAPS2-0", 504),
    ];

    let mut seen_any = false;
    for (base, expected_entries) in sets {
        let dat_path = gamedata_dir().join(format!("{base}.DAT"));
        let tab_path = gamedata_dir().join(format!("{base}.TAB"));
        if !dat_path.exists() {
            eprintln!("note: {base} not present — skipping");
            continue;
        }
        seen_any = true;

        let archive = TmapsArchive::open(
            &std::fs::read(&dat_path).unwrap(),
            &std::fs::read(&tab_path).unwrap(),
        )
        .unwrap_or_else(|e| panic!("{base}: {e}"));
        assert_eq!(archive.entries().len(), expected_entries, "{base}");

        let mut plain = 0usize;
        let mut animated = 0usize;
        let mut irregular = Vec::new();
        let mut empty = 0usize;
        for &entry in archive.entries() {
            // extract() asserts the decompressed size matches the TAB's
            // declared size on every entry.
            let payload = archive
                .extract(entry)
                .unwrap_or_else(|e| panic!("{base} entry {}: {e}", entry.index));
            match archive.texture(entry) {
                Ok(tex) => {
                    plain += 1;
                    assert!(
                        tex.width > 0 && tex.height > 0,
                        "{base} entry {}: degenerate {}x{}",
                        entry.index,
                        tex.width,
                        tex.height
                    );
                }
                // Animated entries (flag bit 0) carry more data than
                // 6 + w*h — an extended layout the sprite track will
                // decode. A handful of retail entries are dead filler
                // with nonsense headers (e.g. MC1 TMAPS1-0 entry 153);
                // more than that means a real parse bug.
                Err(_) => {
                    // One-byte "0" payloads are placeholder slots for
                    // textures absent from this environment's set (MC2's
                    // night TMAPS has runs of them).
                    if payload.len() <= 1 {
                        empty += 1;
                        continue;
                    }
                    let flags = u16::from_le_bytes(payload[0..2].try_into().unwrap());
                    if flags & 1 != 0 {
                        animated += 1;
                    } else {
                        irregular.push(entry.index);
                    }
                }
            }
        }
        assert!(
            irregular.len() <= 2,
            "{base}: too many undecodable non-animated entries: {irregular:?}"
        );
        eprintln!(
            "{base}: {plain} plain, {animated} animated (extended layout), {empty} empty, irregular {irregular:?}"
        );
    }
    assert!(
        seen_any || !gamedata_dir().exists(),
        "gamedata present but no TMAPS found"
    );
}
