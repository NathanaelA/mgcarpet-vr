//! Scratch probe: enumerate TMAPS entries (index, size, flags, group)
//! to see the animation-group structure and size distribution.

use mgc_import::tmaps::TmapsArchive;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dat = std::fs::read(&args[1]).unwrap();
    let tab = std::fs::read(&args[2]).unwrap();
    let dat = if mgc_import::rnc::is_rnc(&dat) {
        mgc_import::rnc::decompress(&dat).unwrap()
    } else {
        dat
    };
    let tab = if mgc_import::rnc::is_rnc(&tab) {
        mgc_import::rnc::decompress(&tab).unwrap()
    } else {
        tab
    };
    let a = TmapsArchive::open(&dat, &tab).unwrap();
    // Dump mode: `tmaps_probe DAT TAB <index> <out.bin>` writes one payload.
    if args.len() == 5 {
        let idx: usize = args[3].parse().unwrap();
        let p = a.extract(a.entries()[idx]).unwrap();
        std::fs::write(&args[4], &p).unwrap();
        println!("entry {idx}: {} bytes -> {}", p.len(), args[4]);
        return;
    }
    println!("{} entries", a.entries().len());
    for e in a.entries() {
        match a.texture(*e) {
            Ok(t) => println!(
                "{:3} group {:3} flags {:04x} {}x{}",
                e.index, e.group, t.flags, t.width, t.height
            ),
            Err(_) => {
                let p = a.extract(*e).unwrap();
                let flags = u16::from_le_bytes(p[0..2].try_into().unwrap());
                let w = u16::from_le_bytes(p[2..4].try_into().unwrap());
                let h = u16::from_le_bytes(p[4..6].try_into().unwrap());
                let expect = 6 + w as usize * h as usize;
                println!(
                    "{:3} group {:3} flags {:04x} {}x{} payload {} (raw {:+}) next16 {:02x?}",
                    e.index,
                    e.group,
                    flags,
                    w,
                    h,
                    p.len(),
                    p.len() as i64 - expect as i64,
                    &p[6..22.min(p.len())]
                );
            }
        }
    }
}
