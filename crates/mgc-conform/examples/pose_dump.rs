//! Dump the human carpet's RECORDED pose per tick (t, x, y, z,
//! action) from an MC2 `.mgcr` — the transit-clustering probe's
//! input: warp ticks show as one-tick x/y jumps far beyond the
//! carpet's top speed.
use mgc_formats::mgcr::Recording;

fn main() {
    let path = std::env::args().nth(1).expect("usage: pose_dump <mgcr>");
    let mut rec = Recording::open(std::path::Path::new(&path)).expect("open");
    while let Some(r) = rec.next_tick() {
        let tick = r.expect("tick");
        let Some(state) = &tick.state else { continue };
        let st = mgc_formats::mgcr::decode_retail_mc2(state).expect("decode");
        let Some(ply) = st.players.get(st.local_player as usize) else {
            continue;
        };
        let slot = ply.play_index as usize;
        let Some(e) = st.ents.get(slot) else { continue };
        println!("{}\t{}\t{}\t{}\t{}", tick.t, e.x, e.y, e.z, e.action45);
    }
}
