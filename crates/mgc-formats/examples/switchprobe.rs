fn main() {
    let f = std::fs::File::open("baked/mc2/level-000.mgcl").unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(f).unwrap();
    for t in &pkg.things.things {
        if t.class == 11 && matches!(t.model, 4 | 16 | 17 | 32) {
            println!(
                "(11,{:2}) slot {:3} at ({:3},{:3}) dis {:5} box {:2} fires {:2} par1 {:3} par2 {}",
                t.model, t.slot, t.x, t.y, t.dis_id, t.swi_sz, t.swi_id, t.parent, t.child
            );
        }
    }
    if let Some(st) = &pkg.stages {
        for (row, c) in st.checkpoints.iter().enumerate() {
            println!(
                "stage row {row}: idx {} stage {} at ({},{})",
                c.index, c.stage, c.x, c.y
            );
        }
    }
}
