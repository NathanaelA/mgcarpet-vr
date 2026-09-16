//! Walk ONE retail slot's chosen fields over a tick window, printing
//! only on CHANGE.
//!
//!     field_walk_mc2 <take.mgcr> <slot> <t0> <t1> [field…]
//!
//! ⭐ Decoded the whole MC2 castle-spell cost ladder in one run
//! (round 138, law 2): the rungs come off the register's record
//! `dword_0x10_16`, and seeing them move is what showed the port was
//! pricing a DIFFERENT castle rather than mis-reading a table.
use mgc_formats::mgcr::{Recording, decode_retail_mc2};
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let slot: usize = a.next().unwrap().parse().unwrap();
    let t0: u64 = a.next().unwrap().parse().unwrap();
    let t1: u64 = a.next().unwrap().parse().unwrap();
    let mut rec = Recording::open(std::path::Path::new(&path)).unwrap();
    let mut prev = String::new();
    while let Some(r) = rec.next_tick() {
        let tick = r.unwrap();
        if tick.t < t0 || tick.t > t1 { continue }
        let Some(raw) = tick.state.as_ref() else { continue };
        let Ok(st) = decode_retail_mc2(raw) else { continue };
        let Some(e) = st.ents.get(slot) else { continue };
        let s = format!("cls=({},{}) act={} f2a={} f30={} mana={} mana_max={} d88={} b46={} maxlife={} b3b={} phase={}",
            e.class3f, e.model40, e.action45, e.f2a, e.f30, e.mana, e.mana_max, e.d88, e.b46, e.max_life, e.b3b, e.phase3e);
        let key = s.split(" phase=").next().unwrap().to_string();
        if key != prev { println!("t={:<6} {}", tick.t, s); prev = key; }
    }
}
