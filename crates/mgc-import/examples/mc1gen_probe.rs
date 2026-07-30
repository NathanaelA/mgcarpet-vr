//! Scratch probe for the native MC1 generator (dev aid, not shipped).
//! Usage: cargo run -p mgc-import --example mc1gen_probe <DAT> <TAB> <index>

use mgc_import::dattab::Archive;
use mgc_import::level_mc1::Mc1Level;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let g = &if args.len() > 4 {
        // raw param mode: seed off raise gnarl river sourc snflt bhlin bhflt rkste
        let p: Vec<i64> = args[1..].iter().map(|a| a.parse().unwrap()).collect();
        mgc_import::level_mc1::GenMap {
            pre_header: 0,
            seed: p[0] as u32,
            off: p[1] as u32,
            raise: p[2] as i32,
            gnarl: p[3] as u32,
            river: p[4] as u32,
            sourc: p[5] as u32,
            snlin: 200,
            snflt: p[6] as u32,
            bhlin: p[7] as u32,
            bhflt: p[8] as u32,
            rkste: p[9] as u32,
        }
    } else {
        let dat = std::fs::read(&args[1]).unwrap();
        let tab = std::fs::read(&args[2]).unwrap();
        let index: usize = args[3].parse().unwrap();
        let archive = Archive::open(&dat, &tab).unwrap();
        let entry = archive.non_empty().find(|e| e.index == index).unwrap();
        let level = Mc1Level::parse(&archive.extract(entry).unwrap()).unwrap();
        level.gen_map
    };
    println!("params: {g:?}");
    let t = mgc_import::mc1_terrain::generate(g, false);

    let water = t.height.iter().filter(|&&h| h == 0).count();
    let max_h = t.height.iter().max().unwrap();
    println!("water {:.1}%  max height {}", water as f64 / 655.36, max_h);
    let mut hist = [0u32; 256];
    for &ty in &t.tile_type {
        hist[ty as usize] += 1;
    }
    print!("types: ");
    for (ty, &n) in hist.iter().enumerate() {
        if n > 0 {
            print!("{ty}:{n} ");
        }
    }
    println!();
    let deep = t.angle.iter().filter(|&&a| a & 8 != 0).count();
    let oriented = t.angle.iter().filter(|&&a| a & 0x70 != 0).count();
    println!("deep-water {deep}  oriented {oriented}");
}
