//! TEMP probe: goat-basin fencing + causeway shore slope field on level-000.
fn cap_bit(t: u8) -> u32 {
    match t {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 0x10,
        5 => 0x20,
        8 => 0x100,
        9 => 0x200,
        10 => 0x100000,
        11 => 0x200000,
        12 => 0x400000,
        13 | 14 => 0,
        15..=20 | 28..=34 => 0x400,
        21 | 22 | 24 => 0x20000,
        23 => 0x40000,
        25 | 27 => 0x80000,
        26 => 0x10000,
        _ => 0x800000,
    }
}
fn rough(h: &[u8], x: u8, y: u8) -> i32 {
    let g = |dx: u8, dy: u8| {
        h[((y.wrapping_add(dy) as usize) << 8) | x.wrapping_add(dx) as usize] as i32
    };
    let (p1, p2, p3, p4) = (g(0, 0), g(1, 0), g(1, 1), g(0, 1));
    (p1 + p4 - p2 - p3).abs().max((p1 + p2 - p4 - p3).abs())
}
fn dump(h: &[u8], t: &[u8], x0: u8, x1: u8, y0: u8, y1: u8, mask: u32, v16: i32, label: &str) {
    println!("== {label} (x {x0}..{x1}, y {y0}..{y1}) mask ~{mask:#010x} v16 {v16} ==");
    println!(
        "legend: ~ water(t0)  # mask-blocked-type  ^ slope>=v16  + both  . passable  digits=height/8"
    );
    for y in y0..=y1 {
        let mut row = String::new();
        for x in x0..=x1 {
            let ty = t[((y as usize) << 8) | x as usize];
            let mb = !mask & cap_bit(ty) != 0;
            let sb = rough(h, x, y) >= v16;
            row.push(if ty == 0 {
                '~'
            } else if mb && sb {
                '+'
            } else if mb {
                '#'
            } else if sb {
                '^'
            } else {
                '.'
            });
        }
        println!("{y:3} {row}");
    }
    // Height strip for the same rows, /8 to one digit
    println!("-- heights/8 --");
    for y in y0..=y1 {
        let mut row = String::new();
        for x in x0..=x1 {
            let hh = h[((y as usize) << 8) | x as usize] / 8;
            row.push(char::from_digit((hh as u32).min(9), 10).unwrap());
        }
        println!("{y:3} {row}");
    }
}
fn main() {
    let f = std::fs::File::open("baked/mc2/level-000.mgcl").expect("bake");
    let pkg = mgc_formats::mgcl::read(f).expect("read");
    let terr = pkg.terrain.expect("terrain");
    let (h, t) = (&terr.height, &terr.tile_type);
    // Goat: mask 0xfff080fe, v16 20. Herd basin.
    dump(h, t, 30, 120, 8, 55, 0xfff080fe, 20, "GOAT basin");
    // Villager: mask 0xfffffefe, v16 15. Causeway west shore + start village.
    dump(
        h,
        t,
        85,
        150,
        195,
        230,
        0xfffffefe,
        15,
        "VILLAGER causeway west + start village",
    );
    // East end of causeway to eastern ring.
    dump(
        h,
        t,
        140,
        185,
        195,
        235,
        0xfffffefe,
        15,
        "VILLAGER causeway east + eastern ring",
    );
}
