//! Integration test against real game data.
//!
//! Runs only when original game files are present under `gamedata/`
//! (see gamedata/README.md); silently passes otherwise so CI and
//! asset-less checkouts stay green. Point `MGC_GAMEDATA` elsewhere to
//! override the location.
//!
//! Every RNC container found must decompress with both CRCs verifying —
//! the container carries its own ground truth, so this is a real
//! correctness check of the decompressor against Bullfrog's packer.

use std::path::{Path, PathBuf};

use mgc_import::rnc;

fn gamedata_dir() -> PathBuf {
    match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn all_gamedata_rnc_files_decompress() {
    let root = gamedata_dir();
    let mut files = Vec::new();
    collect_files(&root, &mut files);

    let mut checked = 0u32;
    let mut failures = Vec::new();
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        if !rnc::is_rnc(&data) {
            continue;
        }
        checked += 1;
        if let Err(e) = rnc::decompress(&data) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }

    if checked == 0 {
        eprintln!(
            "note: no RNC files under {} — install game data to enable this test",
            root.display()
        );
        return;
    }
    eprintln!("verified {checked} RNC containers");
    assert!(
        failures.is_empty(),
        "RNC failures:\n{}",
        failures.join("\n")
    );
}
