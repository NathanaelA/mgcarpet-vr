//! SCRATCH-LANE CENSUS (round 160): what does retail's pool slot 0
//! `+4` actually do across a take? Slot 0 is the collapse scratch
//! (`features::SCRATCH`) — `sub_28FE0` spends its own LCG stream, two
//! draws per knocked wall cell, and NO graded channel watches it
//! (`verify.rs` skips slot 0, and every class-0 record besides).
//!
//! Prints one row per state-bearing tick whose slot-0 `rand` moved,
//! with the LCG distance (= retail's exact draw count for that tick),
//! plus a per-take summary. Usage: scratch_lane_census_mc1 <mgcr>...
use mgc_formats::mgcr::Recording;

/// Slot 0's `+4` straight out of the raw state image — the full
/// `decode_retail_mc1` walks all 1,000 records and is ~100x the cost.
fn scratch_rand(d: &[u8]) -> u32 {
    // mgcr.rs `m1`: POOL = 29_795, record RNG = +4 (module is private).
    let o = 29_795 + 4;
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Steps from `a` to `b` under the record LCG (9377/9439), or `None`
/// if `b` is not reachable within `cap` draws.
fn lcg_dist(a: u32, b: u32, cap: u32) -> Option<u32> {
    let mut r = a;
    for n in 0..=cap {
        if r == b {
            return Some(n);
        }
        r = r.wrapping_mul(9377).wrapping_add(9439);
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    for path in args.iter().filter(|a| !a.starts_with('-')) {
        let Ok(mut rec) = Recording::open(std::path::Path::new(path)) else {
            println!("{path}: OPEN FAILED");
            continue;
        };
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let (mut prev, mut states, mut moves, mut draws, mut unreach) =
            (None, 0u64, 0u64, 0u64, 0u64);
        let mut first = None;
        while let Some(r) = rec.next_tick() {
            let tick = r.expect("tick");
            let Some(state) = &tick.state else { continue };
            states += 1;
            let cur = scratch_rand(state);
            if let Some((pt, pr)) = prev {
                if pr != cur {
                    moves += 1;
                    match lcg_dist(pr, cur, 20_000) {
                        Some(n) => {
                            draws += n as u64;
                            if first.is_none() {
                                first = Some((tick.t, n));
                            }
                            if verbose {
                                println!(
                                    "  {name} t={} (prev {pt}) rand {pr} -> {cur} draws={n}",
                                    tick.t
                                );
                            }
                        }
                        None => {
                            unreach += 1;
                            if verbose {
                                println!(
                                    "  {name} t={} (prev {pt}) rand {pr} -> {cur} UNREACHABLE",
                                    tick.t
                                );
                            }
                        }
                    }
                }
            }
            prev = Some((tick.t, cur));
        }
        let last = prev.map(|p| p.1).unwrap_or(0);
        println!(
            "{name}: states={states} moved_ticks={moves} total_draws={draws} \
             unreachable={unreach} first={first:?} final_rand={last}"
        );
    }
}
