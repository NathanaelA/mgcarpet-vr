//! Census of MC2 objective types + stage-var kinds across ALL baked
//! levels — the "what's still unported" exposure map for the stage
//! engine. Prints, per objective type / stage-var kind, how many levels
//! use it and which. Usage: cargo run -p mgc-sim --example tmp_objcensus
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked/mc2");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    files.sort();

    // objective type -> (level count, sample levels)
    let mut obj: BTreeMap<i8, (usize, Vec<String>)> = BTreeMap::new();
    let mut svar: BTreeMap<i8, (usize, Vec<String>)> = BTreeMap::new();
    let mut n_levels = 0usize;

    for f in &files {
        let Ok(file) = std::fs::File::open(f) else {
            continue;
        };
        let Ok(pkg) = mgc_formats::mgcl::read(file) else {
            continue;
        };
        n_levels += 1;
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let Some(st) = &pkg.stages else { continue };
        let mut types_here = std::collections::BTreeSet::new();
        for c in &st.checkpoints {
            if c.index >= 0 {
                types_here.insert(c.index);
            }
        }
        for t in types_here {
            let e = obj.entry(t).or_default();
            e.0 += 1;
            if e.1.len() < 6 {
                e.1.push(name.clone());
            }
        }
        let mut kinds_here = std::collections::BTreeSet::new();
        for v in &st.variables {
            // StageVar KIND = low nibble of index (index 0 = unused).
            let k = v.index & 0xF;
            if k != 0 {
                kinds_here.insert(k);
            }
        }
        for k in kinds_here {
            let e = svar.entry(k).or_default();
            e.0 += 1;
            if e.1.len() < 6 {
                e.1.push(name.clone());
            }
        }
    }

    let ported_obj = [0i8, 3, 5, 7, 8, 9];
    println!("== OBJECTIVE TYPES across {n_levels} MC2 levels ==");
    println!("type  levels  ported?  sample");
    for (t, (n, ex)) in &obj {
        let p = if ported_obj.contains(t) { "YES" } else { "NO " };
        println!("  {t:>2}   {n:>4}    {p}     {}", ex.join(","));
    }
    println!("\n== STAGE-VAR KINDS (whole subsystem UNPORTED) ==");
    println!("kind  levels  sample");
    for (k, (n, ex)) in &svar {
        println!("  {k:>2}   {n:>4}    {}", ex.join(","));
    }

    // Distinct levels touched by an UNPORTED objective type (1 or 2) —
    // the "level can't complete" exposure, and by ANY stage-var.
    let mut obj_unported_levels = std::collections::BTreeSet::new();
    let mut svar_levels = std::collections::BTreeSet::new();
    for f in &files {
        let Ok(file) = std::fs::File::open(f) else {
            continue;
        };
        let Ok(pkg) = mgc_formats::mgcl::read(file) else {
            continue;
        };
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let Some(st) = &pkg.stages else { continue };
        if st.checkpoints.iter().any(|c| matches!(c.index, 1 | 2)) {
            obj_unported_levels.insert(name.clone());
        }
        if st.variables.iter().any(|v| v.index & 0xF != 0) {
            svar_levels.insert(name);
        }
    }
    println!(
        "\nDISTINCT levels with an UNPORTED objective type (1|2): {}",
        obj_unported_levels.len()
    );
    println!(
        "  {}",
        obj_unported_levels
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "\nDISTINCT levels touching the stage-var subsystem: {}",
        svar_levels.len()
    );
}
