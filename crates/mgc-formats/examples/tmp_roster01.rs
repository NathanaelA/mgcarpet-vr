//! Scratch probe (temporary): one level's THING roster — entity
//! (class, model) histogram + all class-10 rows in detail. Args:
//! [baked-root] [relative-level-path].
use std::collections::BTreeMap;

fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or("baked".into()));
    let lvl = std::env::args()
        .nth(2)
        .unwrap_or("mc1/level-001.mgcl".into());
    let file = std::fs::File::open(root.join(&lvl)).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let mut hist: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    for t in &pkg.things.things {
        if t.kind != mgc_formats::ThingKind::Entity {
            continue;
        }
        *hist.entry((t.class, t.model)).or_default() += 1;
    }
    println!("=== {lvl} entity (class,model) histogram ===");
    for ((c, m), n) in &hist {
        println!("class {c:2} model {m:3} x{n}");
    }
    if let Some(st) = &pkg.stages {
        println!("=== stage checkpoints ===");
        for (row, c) in st.checkpoints.iter().enumerate() {
            println!(
                "row {row}: index {} stage {} at ({},{})",
                c.index, c.stage, c.x, c.y
            );
        }
        println!("=== stage variables ===");
        for (row, v) in st.variables.iter().enumerate() {
            println!(
                "var {row}: index {} stage {} at ({},{}) data {:#x}",
                v.index, v.stage, v.x, v.y, v.data
            );
        }
    }
    println!("=== class-11 (triggers) + class-5 rows ===");
    for t in &pkg.things.things {
        if t.kind == mgc_formats::ThingKind::Entity && matches!(t.class, 5 | 11) {
            println!(
                "slot {:3} ({},{:2}) at ({},{}) dis {} swi {}/{} parent {} child {} par3 {:?}",
                t.slot, t.class, t.model, t.x, t.y, t.dis_id, t.swi_sz, t.swi_id, t.parent,
                t.child, t.par3
            );
        }
    }
    println!("=== class-10 rows in detail ===");
    for t in &pkg.things.things {
        if t.kind == mgc_formats::ThingKind::Entity && t.class == 10 {
            println!(
                "slot {:3} (10,{:2}) at ({},{}) dis {} swi {}/{} parent {} child {}",
                t.slot, t.model, t.x, t.y, t.dis_id, t.swi_sz, t.swi_id, t.parent, t.child
            );
        }
    }
}
