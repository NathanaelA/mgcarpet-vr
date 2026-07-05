//! Integration test against real game data.
//!
//! Runs only when original game files are present under `gamedata/`
//! (see gamedata/README.md); silently passes otherwise so CI and
//! asset-less checkouts stay green. Point `MGC_GAMEDATA` elsewhere to
//! override the location.
//!
//! Every RNC container reachable through the game sources — including
//! files living inside the GOG CD images — must decompress with both
//! CRCs verifying. The container carries its own ground truth, so this
//! is a real correctness check of the decompressor against Bullfrog's
//! packer (and of the ISO reader's byte fidelity along the way).

use std::path::{Path, PathBuf};

use mgc_import::gamedata::Gamedata;
use mgc_import::rnc;

fn gamedata() -> Gamedata {
    let root = match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    };
    Gamedata::locate(&root)
}

#[test]
fn all_gamedata_rnc_files_decompress() {
    let found = gamedata();

    let mut checked = 0u32;
    let mut failures = Vec::new();
    for (tag, src) in [("mc1", &found.mc1), ("mc2", &found.mc2)] {
        let Some(src) = src else {
            continue;
        };
        for rel in src.list() {
            let data = src
                .read(&rel)
                .unwrap_or_else(|e| panic!("{tag} {rel}: listed but unreadable: {e}"));
            if !rnc::is_rnc(&data) {
                continue;
            }
            checked += 1;
            if let Err(e) = rnc::decompress(&data) {
                failures.push(format!("{tag} {rel}: {e}"));
            }
        }
    }

    if checked == 0 {
        eprintln!("note: no RNC files found — install game data to enable this test");
        return;
    }
    eprintln!("verified {checked} RNC containers");
    assert!(
        failures.is_empty(),
        "RNC failures:\n{}",
        failures.join("\n")
    );
}
