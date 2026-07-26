//! What does mc1:24's start trigger actually fire? Dump level-024's
//! authored THINGs — the (10,17) blast rings, triggers, and their
//! dispositions — to size the scripted meteor barrage behind the
//! c10m0×2311 pool flood (player report 2026-07-27).
//! Usage: cargo run -p mgc-sim --example tmp_l24meteor
use std::path::Path;

fn main() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked/mc1/level-024.mgcl");
    let pkg = mgc_formats::mgcl::read(std::fs::File::open(&p).unwrap()).unwrap();
    let mut by_cm: std::collections::BTreeMap<(u16, u16), usize> = Default::default();
    for t in &pkg.things.things {
        *by_cm.entry((t.class, t.model)).or_default() += 1;
    }
    println!("== thing census (class, model) x count ==");
    for ((c, m), n) in &by_cm {
        println!("  c{c} m{m} x{n}");
    }
    println!("== class-10 model-17 (blast rings) ==");
    for t in &pkg.things.things {
        if t.class == 10 && t.model == 17 {
            println!("  {t:?}");
        }
    }
    println!("== class-11 (triggers) ==");
    for t in &pkg.things.things {
        if t.class == 11 {
            println!("  {t:?}");
        }
    }
    println!("== class-9 (projectiles, authored) ==");
    for t in &pkg.things.things {
        if t.class == 9 {
            println!("  {t:?}");
        }
    }
}
