//! `mgc-import` — command-line importer/baker.
//!
//! Current commands operate at the container level (RNC); format-aware
//! import (levels, sprites, sounds) will grow on top.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mgc_import::{dattab, rnc};

/// Print one line to stdout; false when the reader went away (e.g.
/// piped into `head`), so bulk listings can stop instead of panicking
/// on the broken pipe.
fn out(line: std::fmt::Arguments) -> bool {
    writeln!(std::io::stdout(), "{line}").is_ok()
}

macro_rules! outln {
    ($($arg:tt)*) => {
        if !out(format_args!($($arg)*)) {
            return ExitCode::SUCCESS;
        }
    };
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") if args.len() == 2 => scan(Path::new(&args[1])),
        Some("unpack") if args.len() == 2 || args.len() == 3 => {
            unpack(Path::new(&args[1]), args.get(2).map(PathBuf::from))
        }
        Some("archive") if args.len() == 3 => archive(Path::new(&args[1]), Path::new(&args[2])),
        Some("extract") if args.len() == 5 => extract(
            Path::new(&args[1]),
            Path::new(&args[2]),
            &args[3],
            Path::new(&args[4]),
        ),
        _ => {
            eprintln!("mgc-import — Magic Carpet data importer/baker\n");
            eprintln!("Usage:");
            eprintln!(
                "  mgc-import scan <dir>             recursively find and verify RNC containers"
            );
            eprintln!(
                "  mgc-import unpack <file> [out]    decompress one RNC file (default: <file>.unpacked)"
            );
            eprintln!("  mgc-import archive <DAT> <TAB>    list a DAT/TAB archive's entries");
            eprintln!("  mgc-import extract <DAT> <TAB> <index> <out>  extract one archive entry");
            ExitCode::from(2)
        }
    }
}

fn open_archive(dat_path: &Path, tab_path: &Path) -> Result<dattab::Archive, ExitCode> {
    let read = |p: &Path| {
        std::fs::read(p).map_err(|e| {
            eprintln!("error: cannot read {}: {e}", p.display());
            ExitCode::FAILURE
        })
    };
    dattab::Archive::open(&read(dat_path)?, &read(tab_path)?).map_err(|e| {
        eprintln!("error: {}: {e}", dat_path.display());
        ExitCode::FAILURE
    })
}

fn archive(dat_path: &Path, tab_path: &Path) -> ExitCode {
    let archive = match open_archive(dat_path, tab_path) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let mut failed = 0u32;
    let mut shown = 0u32;
    for entry in archive.non_empty() {
        shown += 1;
        let raw = archive.raw(entry);
        if rnc::is_rnc(raw) {
            match archive.extract(entry) {
                Ok(out) => outln!(
                    "  {:4}  @{:<8}  {:>8} -> {:>8} bytes (RNC)",
                    entry.index,
                    entry.offset,
                    entry.len,
                    out.len()
                ),
                Err(e) => {
                    failed += 1;
                    outln!(
                        "  {:4}  @{:<8}  {:>8} bytes  FAIL: {e}",
                        entry.index,
                        entry.offset,
                        entry.len
                    );
                }
            }
        } else {
            outln!(
                "  {:4}  @{:<8}  {:>8} bytes (raw)",
                entry.index,
                entry.offset,
                entry.len
            );
        }
    }
    println!(
        "\n{} entries ({} non-empty, {} extraction failures)",
        archive.entries().len(),
        shown,
        failed
    );
    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn extract(dat_path: &Path, tab_path: &Path, index: &str, out_path: &Path) -> ExitCode {
    let archive = match open_archive(dat_path, tab_path) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let Ok(index) = index.parse::<usize>() else {
        eprintln!("error: bad index {index}");
        return ExitCode::FAILURE;
    };
    let Some(entry) = archive.entries().get(index).copied() else {
        eprintln!(
            "error: index {index} out of range (0..{})",
            archive.entries().len()
        );
        return ExitCode::FAILURE;
    };
    match archive.extract(entry) {
        Ok(out) => {
            if let Err(e) = std::fs::write(out_path, &out) {
                eprintln!("error: cannot write {}: {e}", out_path.display());
                return ExitCode::FAILURE;
            }
            println!(
                "entry {index} -> {} ({} bytes)",
                out_path.display(),
                out.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: entry {index}: {e}");
            ExitCode::FAILURE
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
                outln!(
                    "  OK     {}  {} -> {} bytes (method {})",
                    path.display(),
                    data.len(),
                    out.len(),
                    rnc::parse_header(&data).map(|h| h.method).unwrap_or(0),
                );
            }
            Err(e) => {
                rnc_bad += 1;
                outln!("  FAIL   {}  {e}", path.display());
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
