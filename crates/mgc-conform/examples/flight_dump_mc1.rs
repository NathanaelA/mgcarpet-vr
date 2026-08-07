//! Dump the human's RECORDED MC1 flight column per tick — the pose
//! channel's triage microscope: t, +63 clock, consumed move byte
//! (T160 dw_0), target/actual speed, strafe, stick accumulators,
//! eff_pitch, pose. Usage: flight_dump_mc1 <mgcr> [t0 t1]
use mgc_formats::mgcr::Recording;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: flight_dump_mc1 <mgcr> [t0 t1]");
    let t0: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let t1: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX);
    let mut rec = Recording::open(std::path::Path::new(&path)).expect("open");
    println!("t\tf63\tmb\ttgt\tact\tstrafe\troll\tpitch\teffp\tx\ty\tz\tyaw\taimp\tkmag\tkdir");
    while let Some(r) = rec.next_tick() {
        let tick = r.expect("tick");
        if tick.t < t0 || tick.t > t1 {
            continue;
        }
        let Some(state) = &tick.state else { continue };
        let st = mgc_formats::mgcr::decode_retail_mc1(state).expect("decode");
        let Some(w) = st.wizards.get(st.local_player as usize) else {
            continue;
        };
        let Some(e) = st.ents.get(w.play_index as usize) else {
            continue;
        };
        println!(
            "{}\t{}\t{:#x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            tick.t,
            e.f63,
            w.move_bits,
            w.cmd_speed,
            e.f126,
            w.strafe,
            w.roll_acc as i16,
            w.pitch_acc as i16,
            w.eff_pitch,
            e.x,
            e.y,
            e.z,
            e.f30,
            e.f32,
            w.knock_mag,
            w.knock_dir
        );
    }
}
