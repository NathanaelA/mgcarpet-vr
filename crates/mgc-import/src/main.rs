//! `mgc-import` — command-line importer/baker.
//!
//! Current commands operate at the container level (RNC); format-aware
//! import (levels, sprites, sounds) will grow on top.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mgc_import::{bake, dattab, level_mc1, rnc};

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
        Some("level") if args.len() == 4 => {
            level(Path::new(&args[1]), Path::new(&args[2]), &args[3])
        }
        Some("bake") if args.len() == 3 => bake_cmd(Path::new(&args[1]), Path::new(&args[2])),
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
            eprintln!("  mgc-import level <DAT> <TAB> <index>  inspect an MC1 level");
            eprintln!("  mgc-import bake <gamedata> <out>  bake all levels into .mgcl packages");
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

fn level(dat_path: &Path, tab_path: &Path, index: &str) -> ExitCode {
    let archive = match open_archive(dat_path, tab_path) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let Ok(index) = index.parse::<usize>() else {
        eprintln!("error: bad index {index}");
        return ExitCode::FAILURE;
    };
    let Some(entry) = archive.entries().get(index).copied() else {
        eprintln!("error: index {index} out of range");
        return ExitCode::FAILURE;
    };
    let payload = match archive.extract(entry) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: entry {index}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let level = match level_mc1::Mc1Level::parse(&payload) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: entry {index}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let g = &level.gen_map;
    println!(
        "GEN_MAP  seed={} off={} raise={} gnarl={}",
        g.seed, g.off, g.raise, g.gnarl
    );
    println!(
        "         river={} sourc={} snlin={} snflt={}",
        g.river, g.sourc, g.snlin, g.snflt
    );
    println!(
        "         bhlin={} bhflt={} rkste={} (pre-header {})",
        g.bhlin, g.bhflt, g.rkste, g.pre_header
    );
    println!("footer   {:?}", level.footer);
    if level.reserved_nonzero {
        println!("note: reserved block is NOT all zeros");
    }

    let mut census = std::collections::BTreeMap::<(u16, u16), u32>::new();
    for (_, thing) in level.active_things() {
        *census.entry((thing.class, thing.model)).or_default() += 1;
    }
    println!(
        "\nentities ({} active, {} markers, {} junk slots):",
        level.active_things().count(),
        level.markers().count(),
        level.junk().count()
    );
    for ((class, model), count) in &census {
        outln!("  {:>4}  {}", count, level_mc1::thing_name(*class, *model));
    }
    ExitCode::SUCCESS
}

fn bake_cmd(gamedata: &Path, out_dir: &Path) -> ExitCode {
    use mgc_formats::Game;
    let found = mgc_import::gamedata::Gamedata::locate(gamedata);
    match &found.mc1 {
        Some(src) => println!("mc1 source: {}", src.origin),
        None => eprintln!("note: no MC1 data under {} — skipping", gamedata.display()),
    }
    match &found.mc2 {
        Some(src) => println!("mc2 source: {}", src.origin),
        None => eprintln!("note: no MC2 data under {} — skipping", gamedata.display()),
    }

    // MC1 terrain is generated natively (mc1_terrain); only MC2 needs
    // the remc2-carved oracle tool.
    let genlevel = bake::find_genlevel();
    match &genlevel {
        Some(tool) => println!("mc2 terrain oracle: {}", tool.display()),
        None => println!(
            "mc2 terrain oracle not found (build tools/mc2-genlevel or set MGC_GENLEVEL) — baking mc2 without terrain"
        ),
    }

    let mut manifest = Vec::new();
    if let Some(src) = &found.mc1 {
        let archives = [
            (Game::MagicCarpet1, "mc1", "LEVELS/LEVELS"),
            (Game::HiddenWorlds, "mc1hw", "LEVELS/DDLEVELS"),
        ];
        for (game, tag, base) in archives {
            if !src.exists(&format!("{base}.DAT")) {
                eprintln!("note: {base}.DAT not found — skipping {tag}");
                continue;
            }
            match bake::bake_mc1_archive(game, tag, src, base, out_dir) {
                Ok(outputs) => {
                    println!("{tag}: baked {} levels", outputs.len());
                    manifest.extend(outputs);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        if src.exists("DATA/PAL0-0.DAT") {
            match mgc_import::bundle::bake_mc1_bundles(src, out_dir) {
                Ok(outputs) => {
                    println!(
                        "mc1: baked asset bundles mc1-temperate + mc1-arctic ({} members)",
                        outputs.len()
                    );
                    manifest.extend(outputs);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            eprintln!("note: mc1 DATA/PAL0-0.DAT not found — skipping asset bundles");
        }
    }

    if let Some(src) = &found.mc2 {
        match bake::bake_mc2_archive(src, out_dir, genlevel.as_deref()) {
            Ok((outputs, skipped)) => {
                println!("mc2: baked {} levels", outputs.len());
                if !skipped.is_empty() {
                    println!(
                        "mc2: skipped {} extended-format dev leftovers (indices {:?})",
                        skipped.len(),
                        skipped
                    );
                }
                manifest.extend(outputs);
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
        // Environment bundles need the CD catalogs (absent from
        // hard-disk-only legacy copies).
        if src.exists("DATA/PALD-0.DAT") {
            match mgc_import::bundle::bake_mc2_bundles(src, out_dir) {
                Ok(outputs) => {
                    println!(
                        "mc2: baked asset bundles mc2-day/night/night-fog/cave ({} members)",
                        outputs.len()
                    );
                    manifest.extend(outputs);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            eprintln!(
                "note: mc2 DATA/PALD-0.DAT not found (CD catalogs missing) — skipping mc2 bundles"
            );
        }
    }

    // Any subset of the three games is valid, including none at all
    // (each archive above is skipped with a note when absent).
    if manifest.is_empty() {
        println!(
            "0 packages baked — no game data found under {}",
            gamedata.display()
        );
        return ExitCode::SUCCESS;
    }

    manifest.sort();
    let manifest_path = out_dir.join("manifest.sha256");
    let body: String = manifest
        .iter()
        .map(|(name, hash)| format!("{hash}  {name}\n"))
        .collect();
    if let Err(e) = std::fs::write(&manifest_path, body) {
        eprintln!("error: cannot write {}: {e}", manifest_path.display());
        return ExitCode::FAILURE;
    }
    println!(
        "{} packages, manifest: {}",
        manifest.len(),
        manifest_path.display()
    );
    ExitCode::SUCCESS
}

fn out_scan_line(label: &str, data: &[u8], decompressed: &[u8]) -> bool {
    out(format_args!(
        "  OK     {label}  {} -> {} bytes (method {})",
        data.len(),
        decompressed.len(),
        rnc::parse_header(data).map(|h| h.method).unwrap_or(0),
    ))
}

fn scan(root: &Path) -> ExitCode {
    let mut files = Vec::new();
    if let Err(e) = collect_files(root, &mut files) {
        eprintln!("error: cannot read {}: {e}", root.display());
        return ExitCode::FAILURE;
    }
    files.sort();

    let (mut rnc_ok, mut rnc_bad, mut other) = (0u32, 0u32, 0u32);
    let mut check = |label: &str, data: &[u8]| -> bool {
        if !rnc::is_rnc(data) {
            other += 1;
            return true;
        }
        match rnc::decompress(data) {
            Ok(out) => {
                rnc_ok += 1;
                out_scan_line(label, data, &out)
            }
            Err(e) => {
                rnc_bad += 1;
                out(format_args!("  FAIL   {label}  {e}"))
            }
        }
    };
    for path in &files {
        let Ok(data) = std::fs::read(path) else {
            eprintln!("  ERROR  {} (unreadable)", path.display());
            continue;
        };
        if !check(&path.display().to_string(), &data) {
            return ExitCode::SUCCESS;
        }
        // CD images (the GOG installs' game.gog) are scanned inside-out
        // too — most game data lives there, not on the filesystem.
        if let Ok(image) = mgc_import::iso::IsoImage::open(path) {
            let inner: Vec<String> = image.paths().map(String::from).collect();
            for rel in inner {
                let Ok(data) = image.read(&rel) else {
                    eprintln!("  ERROR  {}!{rel} (unreadable)", path.display());
                    continue;
                };
                if !check(&format!("{}!{rel}", path.display()), &data) {
                    return ExitCode::SUCCESS;
                }
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
