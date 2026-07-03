//! `mgc-import` — command-line importer/baker.
//!
//! Current commands operate at the container level (RNC); format-aware
//! import (levels, sprites, sounds) will grow on top.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mgc_import::rnc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") if args.len() == 2 => scan(Path::new(&args[1])),
        Some("unpack") if args.len() == 2 || args.len() == 3 => {
            unpack(Path::new(&args[1]), args.get(2).map(PathBuf::from))
        }
        _ => {
            eprintln!("mgc-import — Magic Carpet data importer/baker\n");
            eprintln!("Usage:");
            eprintln!(
                "  mgc-import scan <dir>             recursively find and verify RNC containers"
            );
            eprintln!(
                "  mgc-import unpack <file> [out]    decompress one RNC file (default: <file>.unpacked)"
            );
            ExitCode::from(2)
        }
    }
}

fn scan(root: &Path) -> ExitCode {
    let mut files = Vec::new();
    if let Err(e) = collect_files(root, &mut files) {
        eprintln!("error: cannot read {}: {e}", root.display());
        return ExitCode::FAILURE;
    }
    files.sort();

    let (mut rnc_ok, mut rnc_bad, mut other) = (0u32, 0u32, 0u32);
    for path in &files {
        let Ok(data) = std::fs::read(path) else {
            eprintln!("  ERROR  {} (unreadable)", path.display());
            continue;
        };
        if !rnc::is_rnc(&data) {
            other += 1;
            continue;
        }
        match rnc::decompress(&data) {
            Ok(out) => {
                rnc_ok += 1;
                println!(
                    "  OK     {}  {} -> {} bytes (method {})",
                    path.display(),
                    data.len(),
                    out.len(),
                    rnc::parse_header(&data).map(|h| h.method).unwrap_or(0),
                );
            }
            Err(e) => {
                rnc_bad += 1;
                println!("  FAIL   {}  {e}", path.display());
            }
        }
    }
    println!(
        "\n{} files scanned: {rnc_ok} RNC ok, {rnc_bad} RNC failed, {other} non-RNC",
        files.len()
    );
    if rnc_bad > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn unpack(input: &Path, output: Option<PathBuf>) -> ExitCode {
    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let out = match rnc::decompress(&data) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let output = output.unwrap_or_else(|| {
        let mut p = input.as_os_str().to_owned();
        p.push(".unpacked");
        PathBuf::from(p)
    });
    if let Err(e) = std::fs::write(&output, &out) {
        eprintln!("error: cannot write {}: {e}", output.display());
        return ExitCode::FAILURE;
    }
    println!(
        "{} -> {} ({} bytes)",
        input.display(),
        output.display(),
        out.len()
    );
    ExitCode::SUCCESS
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
