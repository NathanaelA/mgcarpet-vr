//! One-shot: list MC2 cave levels + their cave-band THING counts.
fn main() {
    let root = std::path::Path::new("baked/mc2");
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    paths.sort();
    let mut caves = 0;
    for p in &paths {
        let Ok(f) = std::fs::File::open(p) else {
            continue;
        };
        let Ok(pkg) = mgc_formats::mgcl::read(f) else {
            continue;
        };
        let Some(h) = pkg.header.as_ref() else {
            continue;
        };
        if h.map_type != mgc_formats::MapType::Cave {
            continue;
        }
        caves += 1;
        let mut band = std::collections::BTreeMap::new();
        for t in &pkg.things.things {
            if (t.class == 10 && (80..=86).contains(&t.model))
                || (t.class == 14 && t.model == 2)
                || (t.class == 2 && t.model == 6)
                || (t.class == 5 && t.model == 24)
            {
                *band.entry((t.class, t.model)).or_insert(0u32) += 1;
            }
        }
        println!(
            "{}: gfx={} things={} band={:?}",
            p.file_name().unwrap().to_string_lossy(),
            h.gfx_type,
            pkg.things.things.len(),
            band
        );
    }
    println!("total cave levels: {caves} / {}", paths.len());
}
