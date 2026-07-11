//! Phase-4.3 misfit-sweep census: scan every baked MC2 level's THING
//! table and tally the (class, model) pairs the current
//! [`GameId::known_thing`] registry does NOT admit — records, levels,
//! and an example level per pair. Mirrors the runtime spawn gate
//! (entity records only; the x/y >= 256 marker rows never spawn).
use mgc_formats::LevelPackage;
use mgc_sim::ids::GameId;
use std::collections::BTreeMap;

fn main() {
    let root = std::path::Path::new("baked/mc2");
    let mut tally: BTreeMap<(u16, u16), (u32, std::collections::BTreeSet<String>)> =
        BTreeMap::new();
    let mut known_records = 0u64;
    let mut total_records = 0u64;
    let mut levels = 0u32;
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    names.sort();
    for path in names {
        let file = std::fs::File::open(&path).unwrap();
        let pkg: LevelPackage = match mgc_formats::mgcl::read(file) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                continue;
            }
        };
        levels += 1;
        let lvl = path.file_stem().unwrap().to_string_lossy().into_owned();
        for t in &pkg.things.things {
            if t.class == 0 || t.x >= 256 || t.y >= 256 {
                continue;
            }
            total_records += 1;
            if GameId::Mc2.known_thing(t.class, t.model) {
                known_records += 1;
            } else {
                let e = tally.entry((t.class, t.model)).or_default();
                e.0 += 1;
                e.1.insert(lvl.clone());
            }
        }
    }
    println!(
        "{levels} levels, {total_records} spawnable records, {known_records} admitted ({:.1}%)",
        100.0 * known_records as f64 / total_records as f64
    );
    println!("\nunknown (class, model): records / levels / examples");
    let mut rows: Vec<_> = tally.into_iter().collect();
    rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for ((c, m), (n, lv)) in rows {
        let ex: Vec<_> = lv.iter().take(4).cloned().collect();
        println!(
            "({c:2},{m:3})  x{n:<5} / {:<3} levels   e.g. {}",
            lv.len(),
            ex.join(", ")
        );
    }
}
