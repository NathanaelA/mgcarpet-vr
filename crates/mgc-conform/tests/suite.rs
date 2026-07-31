//! The conformance suite as a cargo test (docs/CONFORMANCE.md): every
//! committed manifest under `conformance/` replays its fixture pairs
//! against the current sim and enforces the expected statuses. Skips
//! — with a printed note, mirroring the golden tests' baked-data
//! skip — when the manifests, the source recordings, or the baked
//! tree are absent (they are local corpus data, not repo artifacts).

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

#[test]
fn conformance_suite() {
    let root = repo_root();
    let dir = root.join("conformance");
    let manifests: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| Some(e.ok()?.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            // The known-deviation roster lives beside the suite
            // manifests (docs/CONFORMANCE.md) but is not one.
            .filter(|p| p.file_name().is_none_or(|n| n != "known-deviations.json"))
            .collect(),
        Err(_) => {
            println!("SKIP: no conformance/ manifest dir");
            return;
        }
    };
    if manifests.is_empty() {
        println!("SKIP: conformance/ holds no manifests");
        return;
    }
    if !root.join("baked").exists() {
        println!("SKIP: baked data not present");
        return;
    }
    let mut ran = 0;
    for m in &manifests {
        // The recording is referenced relative to the manifest; skip
        // suites whose corpus file is not on this machine.
        let rec: Option<String> = std::fs::read_to_string(m)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| Some(v.get("recording")?.as_str()?.to_string()));
        let Some(rec) = rec else {
            panic!("{}: unreadable manifest", m.display());
        };
        if !m.parent().unwrap().join(&rec).exists() {
            println!("SKIP {}: recording {rec} not present", m.display());
            continue;
        }
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_mgc-conform"))
            .current_dir(&root)
            .arg("fixtures")
            .arg(m)
            .output()
            .expect("spawn mgc-conform");
        print!("{}", String::from_utf8_lossy(&out.stdout));
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
        assert!(
            out.status.success(),
            "conformance suite {} reported regressions or unpromoted fixes",
            m.display()
        );
        ran += 1;
    }
    println!("conformance: {ran} suite(s) enforced");
}
