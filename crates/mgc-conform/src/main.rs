//! `mgc-conform` — the `.mgcr` conformance fixture runner
//! (docs/RECORDING.md "Consumers → The fixture runner").
//!
//! Modes:
//! - `check-decode <file.mgcr>…` — re-decode every tick's raw
//!   `state.struct_b64` through the Rust decoder and demand value
//!   equality with the recording's own `obs` channel. Pins the Rust
//!   decode against the recorder's (the corpus was certified
//!   obs↔state-coherent at record time, so any mismatch is ours).
//! - `verify-deltas <file.mgcr>` — the retail conformance mode:
//!   import the raw state at tick N into a freshly-built world, tick
//!   once, diff the port's obs projection against the recorded obs at
//!   N+1 (adjacent pairs only; gaps break pairing, never the run).

mod jsondiff;
mod verify;

use mgc_formats::mgcr::{Obs, Recording};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: mgc-conform <mode> [args]\n\
         \n\
         modes:\n\
           check-decode <file.mgcr>…      re-decode state, compare vs stored obs\n\
           verify-deltas <file.mgcr>      import state@N, tick, diff obs@N+1\n\
         \n\
         common flags:\n\
           --max-diffs <n>   mismatch paths printed per tick (default 8)\n\
           --limit <n>       stop after n tick records / pairs (default: all)\n\
         verify-deltas flags:\n\
           --baked <dir>     baked tree root (default: baked)\n\
           --pin-pose n|n1   drive the human with the pre- or post-tick\n\
                             recorded pose (default n1, the app's phase)\n\
           --dump <t>        print the full diff of pair t→t+1\n\
           --dump-first      print the first divergent pair in full"
    );
    std::process::exit(2);
}

pub struct Args {
    mode: String,
    files: Vec<PathBuf>,
    pub max_diffs: usize,
    pub limit: Option<u64>,
    pub baked: PathBuf,
    pub pin_pose: String,
    pub dump: Option<u64>,
    pub dump_first: bool,
    /// Feed the input channel k ticks late (retail's mouse→control→
    /// consume pipeline shows ~2-3 ticks of latency vs the sampled
    /// externals).
    pub input_delay: u64,
}

fn parse_args() -> Args {
    let mut a = Args {
        mode: String::new(),
        files: Vec::new(),
        max_diffs: 8,
        limit: None,
        baked: PathBuf::from("baked"),
        pin_pose: "n1".into(),
        dump: None,
        dump_first: false,
        input_delay: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--max-diffs" => {
                a.max_diffs = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--limit" => {
                a.limit = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--baked" => a.baked = it.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--pin-pose" => a.pin_pose = it.next().unwrap_or_else(|| usage()),
            "--dump-first" => a.dump_first = true,
            "--input-delay" => {
                a.input_delay = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--dump" => {
                a.dump = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "-h" | "--help" => usage(),
            _ if a.mode.is_empty() => a.mode = arg,
            _ => a.files.push(PathBuf::from(arg)),
        }
    }
    if a.mode.is_empty() || a.files.is_empty() {
        usage();
    }
    a
}

fn main() {
    let args = parse_args();
    let code = match args.mode.as_str() {
        "check-decode" => args
            .files
            .iter()
            .map(|f| check_decode(f, &args))
            .max()
            .unwrap_or(0),
        "verify-deltas" => args
            .files
            .iter()
            .map(|f| verify::verify_deltas(f, &args))
            .max()
            .unwrap_or(0),
        _ => usage(),
    };
    std::process::exit(code);
}

/// Re-decode every tick's raw struct image and compare against the
/// stored obs channel, value for value. Exit 0 = every tick matched.
fn check_decode(path: &std::path::Path, args: &Args) -> i32 {
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let family = match rec.header.family() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    println!(
        "== {} (game {}, level {:?}, source {})",
        path.display(),
        rec.header.game,
        rec.header.level,
        rec.header.source
    );
    let (mut ticks, mut ok, mut bad, mut skipped) = (0u64, 0u64, 0u64, 0u64);
    while let Some(r) = rec.next_tick() {
        let tick = match r {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  record error: {e}");
                return 2;
            }
        };
        ticks += 1;
        let (Some(state), Some(stored)) = (&tick.state, &tick.obs) else {
            skipped += 1;
            continue;
        };
        let decoded = match Obs::decode(family, state) {
            Ok(o) => o.to_value(),
            Err(e) => {
                eprintln!("  t={}: decode: {e}", tick.t);
                bad += 1;
                continue;
            }
        };
        let diffs = jsondiff::diff(stored, &decoded, args.max_diffs);
        if diffs.is_empty() {
            ok += 1;
        } else {
            bad += 1;
            println!("  t={}: {} mismatch path(s):", tick.t, diffs.len());
            for d in &diffs {
                println!("    {}: stored {} vs decoded {}", d.path, d.want, d.got);
            }
        }
        if let Some(limit) = args.limit {
            if ticks >= limit {
                break;
            }
        }
    }
    println!(
        "  {} ticks: {} ok, {} mismatched, {} without state+obs",
        ticks, ok, bad, skipped
    );
    if bad == 0 { 0 } else { 1 }
}
