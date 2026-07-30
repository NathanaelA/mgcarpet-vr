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

mod fixtures;
mod jsondiff;
mod verify;
mod verify_mc2;

use mgc_formats::mgcr::{Obs, Recording};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: mgc-conform <mode> [args]\n\
         \n\
         modes:\n\
           check-decode <file.mgcr>…      re-decode state, compare vs stored obs\n\
           verify-deltas <file.mgcr>      import state@N, tick, diff obs@N+1\n\
           dump-state <file.mgcr> <t> <slot>…   print raw retail fields of\n\
                                          the given slots at tick t\n\
           extract <file.mgcr> --out <manifest.json>   lift a fixture-suite\n\
                                          manifest (docs/CONFORMANCE.md)\n\
           fixtures <manifest.json>…      run a fixture suite, enforcing\n\
                                          expected statuses\n\
         \n\
         common flags:\n\
           --max-diffs <n>   mismatch paths printed per tick (default 8)\n\
           --limit <n>       stop after n tick records / pairs (default: all)\n\
         extract flags:\n\
           --out <path>          manifest destination (required)\n\
           --sample-every <n>    conforming-pair sampling stride (default 10)\n\
           --max-open <n>        open-exemplar cap (default 24; the suite\n\
                                 doctrine curates further — CONFORMANCE.md)\n\
         fixtures flags:\n\
           --promote         accept fixed fixtures (status → conforming) and\n\
                             refresh drifted signatures, rewriting the manifest\n\
         verify-deltas flags:\n\
           --baked <dir>     baked tree root (default: baked)\n\
           --pin-pose n|n1   drive the human with the pre- or post-tick\n\
                             recorded pose (default n1, the app's phase)\n\
           --dump <t>        print the full diff of pair t→t+1\n\
           --dump-first      print the first divergent pair in full\n\
           --csv <path>      write every per-pair diff as a TSV row\n\
                             (t, kind, slot, class, model, field, want,\n\
                             got, x, y, z — for offline triage)"
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
    pub dump_port: bool,
    pub csv: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub sample_every: u64,
    pub max_open: usize,
    pub promote: bool,
    /// Feed the input channel k ticks late (retail's mouse→control→
    /// consume pipeline shows ~2-3 ticks of latency vs the sampled
    /// externals).
    pub input_delay: u64,
    /// verify-deltas: skip pairs before this tick (windowed triage;
    /// executed pairs are announced on stderr so an aborting pair
    /// self-incriminates).
    pub start: Option<u64>,
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
        dump_port: false,
        csv: None,
        out: None,
        sample_every: 10,
        max_open: 24,
        promote: false,
        input_delay: 0,
        start: None,
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
            "--csv" => a.csv = Some(it.next().map(PathBuf::from).unwrap_or_else(|| usage())),
            "--pin-pose" => a.pin_pose = it.next().unwrap_or_else(|| usage()),
            "--dump-first" => a.dump_first = true,
            "--dump-port" => a.dump_port = true,
            "--promote" => a.promote = true,
            "--out" => a.out = Some(it.next().map(PathBuf::from).unwrap_or_else(|| usage())),
            "--sample-every" => {
                a.sample_every = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--max-open" => {
                a.max_open = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--start" => {
                a.start = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
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
        "dump-state" => dump_state(&args),
        "trace" => trace(&args),
        "extract" => args
            .files
            .iter()
            .map(|f| fixtures::extract(f, &args))
            .max()
            .unwrap_or(0),
        "fixtures" => fixtures::run(&args.files, &args),
        _ => usage(),
    };
    std::process::exit(code);
}

/// Print the raw retail pool fields of the requested slots at one
/// tick — the triage microscope for divergent pairs (`dump-state
/// <file> <t> <slot>…`).
fn dump_state(args: &Args) -> i32 {
    let (path, rest) = match args.files.split_first() {
        Some(p) => p,
        None => usage(),
    };
    let all = rest.iter().any(|p| p.to_str() == Some("all"));
    let mut it = rest.iter().filter_map(|p| p.to_str()?.parse::<u64>().ok());
    let Some(t) = it.next() else { usage() };
    let slots: Vec<u64> = it.collect();
    if slots.is_empty() && !all {
        usage();
    }
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let mc2 = rec.header.family() == Ok(mgc_formats::mgcr::Family::Mc2);
    while let Some(r) = rec.next_tick() {
        let tick = match r {
            Ok(t) => t,
            Err(e) => {
                eprintln!("record error: {e}");
                return 2;
            }
        };
        if tick.t != t {
            continue;
        }
        let Some(state) = &tick.state else {
            eprintln!("t={t}: no state channel");
            return 2;
        };
        if mc2 {
            let st = match mgc_formats::mgcr::decode_retail_mc2(state) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("t={t}: {e}");
                    return 2;
                }
            };
            if all {
                if let Some(p) = st.players.get(st.local_player as usize) {
                    for s in 0..26 {
                        if p.spell_ent[s] == 0 && p.xp_vol[s] == 0 && p.xp_bank[s] == 0 {
                            continue;
                        }
                        println!(
                            "t={t} book spell {s}: ent={} lvl={} sel={} ring={} \
                             xp={}+{}",
                            p.spell_ent[s],
                            p.levels[s],
                            p.sel[s],
                            p.ring[s],
                            p.xp_vol[s],
                            p.xp_bank[s]
                        );
                    }
                }
                for (s, e) in st.ents.iter().enumerate() {
                    if e.class3f == 0 {
                        continue;
                    }
                    println!(
                        "t={t} slot {s}: cm=({},{}) act={} flags={:#x} life={}/{} \
                         pos=({:.2},{:.2},{}) mana={}/{} own={} id={} pe={} \
                         sv=({},{}) tgt={}",
                        e.class3f,
                        e.model40,
                        e.action45,
                        e.flags,
                        e.life,
                        e.max_life,
                        e.x as f64 / 256.0,
                        e.y as f64 / 256.0,
                        e.z,
                        e.mana,
                        e.mana_max,
                        e.owner28,
                        e.f1a,
                        e.player_ent,
                        e.sv1,
                        e.sv2,
                        e.target96
                    );
                }
            }
            for s in &slots {
                println!("t={t} slot {s}: {:#?}", st.ents[*s as usize]);
            }
            return 0;
        }
        let st = match mgc_formats::mgcr::decode_retail_mc1(state) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("t={t}: {e}");
                return 2;
            }
        };
        if all {
            for (s, e) in st.ents.iter().enumerate() {
                if e.class64 == 0 {
                    continue;
                }
                println!(
                    "t={t} slot {s}: cm=({},{}) st={} flags={:#x} life={}/{} \
                     pos=({:.2},{:.2},{}) mana={}/{} own={} id={} chase={}",
                    e.class64,
                    e.model65,
                    e.f70,
                    e.flags,
                    e.act_life,
                    e.max_life,
                    e.x as f64 / 256.0,
                    e.y as f64 / 256.0,
                    e.z,
                    e.f140,
                    e.f136,
                    e.f144,
                    e.id24,
                    e.f146
                );
            }
        }
        for s in &slots {
            println!("t={t} slot {s}: {:#?}", st.ents[*s as usize]);
        }
        return 0;
    }
    eprintln!("t={t}: not in recording");
    2
}

/// Trace one slot's economy fields across a tick range in a single
/// pass (`trace <file> <slot> <t0> <t1>`): per tick — mana(+140),
/// regen(+132), life(+12), f63, flags. Divergence-cadence microscope.
fn trace(args: &Args) -> i32 {
    let (path, rest) = match args.files.split_first() {
        Some(p) => p,
        None => usage(),
    };
    let nums: Vec<u64> = rest
        .iter()
        .filter_map(|p| p.to_str()?.parse::<u64>().ok())
        .collect();
    let [slot, t0, t1] = nums[..] else { usage() };
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let mc2 = rec.header.family() == Ok(mgc_formats::mgcr::Family::Mc2);
    let mut prev_mana: Option<i32> = None;
    while let Some(r) = rec.next_tick() {
        let Ok(tick) = r else { return 2 };
        if tick.t < t0 {
            continue;
        }
        if tick.t > t1 {
            break;
        }
        let Some(state) = &tick.state else { continue };
        if mc2 {
            let Ok(st) = mgc_formats::mgcr::decode_retail_mc2(state) else {
                return 2;
            };
            let e = &st.ents[slot as usize];
            println!(
                "t={} cm=({},{}) act={} b46={} life={}/{} z={} yaw={} \
                 a=({},{}) spd={} f2a={} f2c={} f2e={} f30={} f36={} \
                 b3b={} d88={} mmax={} mana={} rand={:#06x} ph={} \
                 flags={:#x}",
                tick.t,
                e.class3f,
                e.model40,
                e.action45,
                e.b46,
                e.life,
                e.max_life,
                e.z,
                e.yaw,
                e.ayaw,
                e.apitch,
                e.speed,
                e.f2a,
                e.f2c,
                e.f2e,
                e.f30,
                e.f36,
                e.b3b,
                e.d88,
                e.mana_max,
                e.mana,
                e.rand,
                e.phase3e,
                e.flags
            );
            continue;
        }
        let Ok(st) = mgc_formats::mgcr::decode_retail_mc1(state) else {
            return 2;
        };
        let e = &st.ents[slot as usize];
        let d = prev_mana.map(|p| e.f140 - p).unwrap_or(0);
        prev_mana = Some(e.f140);
        println!(
            "t={} mana={} d={:+} f132={} life={} f63={} f63%4={} flags={:#x}",
            tick.t,
            e.f140,
            d,
            e.f132,
            e.act_life,
            e.f63,
            e.f63 % 4,
            e.flags
        );
    }
    0
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
