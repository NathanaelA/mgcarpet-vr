//! TEMP probe: dump a BUILD00 row + the THINGs near a cell of an MC2 level.
use mgc_formats::LevelPackage;
use mgc_sim::engine::features::FeatureAssets;
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let level: u32 = a[1].parse().unwrap();
    let row: usize = a[2].parse().unwrap();
    let (cx, cy): (i32, i32) = (a[3].parse().unwrap(), a[4].parse().unwrap());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join("mc2-night")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let def = assets.build_tab[row];
    let (w, h) = (def.w as usize, def.h as usize);
    println!("BUILD00 row {row}: w={w} h={h} offset={}", def.offset);
    let cells = &assets.build_dat[def.offset as usize..def.offset as usize + 2 * w * h];
    println!("codes:");
    for dy in 0..h {
        let line: Vec<String> = (0..w).map(|dx| { let c = cells[2 * (dy * w + dx)]; if c == 0xff { " --".into() } else { format!("{c:3}") } }).collect();
        println!("  {}", line.join(""));
    }
    println!("pads:");
    for dy in 0..h {
        let line: Vec<String> = (0..w).map(|dx| { let c = cells[2 * (dy * w + dx) + 1]; if c == 0xff { " --".into() } else { format!("{c:3}") } }).collect();
        println!("  {}", line.join(""));
    }
    let file = std::fs::File::open(root.join("mc2").join(format!("level-{level:03}.mgcl"))).unwrap();
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    println!("THINGs within 20 cells of ({cx},{cy}):");
    for (i, t) in pkg.things.things.iter().enumerate() {
        let (tx, ty) = (t.x as i32, t.y as i32);
        if (cx == 999 && matches!(t.class, 3 | 15)) || ((tx - cx).abs() <= 20 && (ty - cy).abs() <= 20) {
            println!("  #{i}: {t:?}");
        }
    }
}
