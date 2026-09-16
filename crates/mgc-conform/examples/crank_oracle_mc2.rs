//! ROLL dig (round 140): retail's OWN whirlwind crank count per tick.
//!
//!     crank_oracle_mc2 <take.mgcr> <t0> <t1>
//!
//! `roll_acc[t] = ladder(roll_acc[t-1], k) + scaled(rollDelta[t])`
//! where `ladder` re-tests `< 256` before each +28 and `rollDelta` is
//! the capture's own `rollDelta_0x4_4`.  Solve for k.  Also lists every
//! live class-10 model-22 whirlwind HEAD and its cell distance to the
//! human carpet.
use mgc_formats::mgcr::{Recording, decode_retail_mc2, RetailMc2};

fn ladder(acc: i16, k: u8) -> i16 {
    let mut v = acc as i32;
    for _ in 0..k {
        if v < 256 { v += 28; }
    }
    v as i16
}

fn scaled(d: i16, ms: u8) -> i16 {
    if ms > 0 { ((d as i32) * (4 - ms as i32) / 4) as i16 } else { d }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let t0: u64 = a.next().unwrap().parse().unwrap();
    let t1: u64 = a.next().unwrap().parse().unwrap();
    let mut rec = Recording::open(std::path::Path::new(&path)).unwrap();
    let mut prev: Option<(u64, RetailMc2)> = None;
    while let Some(r) = rec.next_tick() {
        let tick = r.unwrap();
        let Some(raw) = tick.state.as_ref() else { continue };
        let Ok(st) = decode_retail_mc2(raw) else { continue };
        if tick.t >= t0 && tick.t <= t1 {
            if let Some((pt, pst)) = prev.as_ref() {
                let pp = &pst.players[pst.local_player as usize];
                let cp = &st.players[st.local_player as usize];
                let ci = cp.play_index as usize;
                let pe = pst.ents.get(ci);
                let ce = st.ents.get(ci);
                let a0 = pp.roll_acc as i16;
                let a1 = cp.roll_acc as i16;
                let ms = pp.move_speed;
                let dr = scaled(cp.roll_delta, ms);
                // k solving: with veto the mover skips roll += dr.
                let mut ks: Vec<(u8, &str)> = vec![];
                for k in 0..=8u8 {
                    if ladder(a0, k).wrapping_add(dr) == a1 { ks.push((k, "dr")); }
                    if ladder(a0, k) == a1 && dr != 0 { ks.push((k, "veto")); }
                }
                // whirlwind heads
                let mut ww = vec![];
                if let (Some(_pe), Some(ce)) = (pe, ce) {
                    for (i, e) in st.ents.iter().enumerate() {
                        if e.class3f == 10 && e.model40 == 22 {
                            let dx = (e.x as i32) - (ce.x as i32);
                            let dy = (e.y as i32) - (ce.y as i32);
                            let d2 = dx * dx + dy * dy;
                            ww.push(format!("#{i}@({},{}) d2={d2} life={}", e.x, e.y, e.life));
                        }
                    }
                }
                let (cf30, pf30, cflags, pflags) = (
                    ce.map_or(0, |e| e.f30), pe.map_or(0, |e| e.f30),
                    ce.map_or(0, |e| e.flags), pe.map_or(0, |e| e.flags));
                println!(
                    "t={} ({}→{}) roll_acc {a0} -> {a1} (Δ{}) rollDelta={} ms={ms} k={:?} f30 {pf30}->{cf30} flags {pflags:#x}->{cflags:#x} | ww[{}]: {}",
                    tick.t, pt, tick.t, a1.wrapping_sub(a0), cp.roll_delta, ks, ww.len(), ww.join(" ")
                );
            }
        }
        prev = Some((tick.t, st));
        if tick.t > t1 { break }
    }
}
