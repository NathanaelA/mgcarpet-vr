use std::collections::BTreeMap;
fn main() {
    let file = std::fs::File::open("baked/mc1/level-032.mgcl").unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let mut by_dis: BTreeMap<u16, Vec<&mgc_formats::Thing>> = BTreeMap::new();
    for t in &pkg.things.things {
        if t.kind == mgc_formats::ThingKind::Entity && t.dis_id != 0 && t.dis_id != 0xFFFF {
            by_dis.entry(t.dis_id).or_default().push(t);
        }
    }
    for (dis, things) in &by_dis {
        let mut counts: BTreeMap<(u16, u16), u32> = BTreeMap::new();
        let mut slots = 0usize;
        for t in things {
            *counts.entry((t.class, t.model)).or_default() += 1;
            slots += if t.class == 5 && matches!(t.model, 0 | 3 | 6) { 17 } else { 1 };
        }
        print!("dis {dis:2} ({slots:3} slots): ");
        for ((c, m), n) in &counts {
            print!("{n}x c{c}m{m} ");
        }
        println!();
        for t in things {
            match t.class {
                11 => println!("    trigger model {} at ({},{}) sz {} fires dis {}", t.model, t.x, t.y, t.swi_sz, t.swi_id),
                10 if t.model == 34 => println!("    PORTAL at ({},{}) -> ({}.5,{}.5)", t.x, t.y, t.child, t.parent),
                12 => println!("    jar/mana m{} at ({},{}) swi_id {}", t.model, t.x, t.y, t.swi_id),
                _ => {}
            }
        }
    }
}
