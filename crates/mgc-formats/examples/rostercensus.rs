//! Census of authored (class, model) THING records across every baked
//! MC2 level — the Phase-4.3 roster-sweep worklist.
use std::collections::BTreeMap;

fn main() {
    let mut census: BTreeMap<(u16, u16), (usize, Vec<String>)> = BTreeMap::new();
    let mut paths: Vec<_> = std::fs::read_dir("baked/mc2")
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    paths.sort();
    for p in &paths {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let Ok(f) = std::fs::File::open(p) else {
            continue;
        };
        let Ok(pkg) = mgc_formats::mgcl::read(f) else {
            eprintln!("unreadable: {name}");
            continue;
        };
        for t in &pkg.things.things {
            let e = census.entry((t.class, t.model)).or_default();
            e.0 += 1;
            if !e.1.contains(&name) {
                e.1.push(name.clone());
            }
        }
    }
    for ((c, m), (n, levels)) in &census {
        let lv = if levels.len() > 6 {
            format!("{} levels", levels.len())
        } else {
            levels.join(",")
        };
        println!("({c:2},{m:3}) x{n:5}  {lv}");
    }
}
