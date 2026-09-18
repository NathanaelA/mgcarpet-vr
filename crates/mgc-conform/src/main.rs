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

mod alloc_trace;
mod explain;
mod fixtures;
mod blob_census;
mod init_check;
mod jsondiff;
mod pose_lane;
mod replay;
mod roster;
mod shadow;
mod slice;
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
           terrain-diff <file.mgcr>…      diff the take's measured terrain base\n\
                                          against the port's generated planes\n\
                                          (the record-0 stock-bake validator)\n\
           blob-census <file.mgcr>…       WHICH BYTES OF THE RAW STRUCT IMAGE EVER\n\
                                          MOVE: per-offset change counts over the\n\
                                          take, pool/wizard records folded onto\n\
                                          their stride, each offset tagged DECODED\n\
                                          or UNDECODED (a lane no decoder lifts is\n\
                                          in no channel). MC1 only\n\
           init-check <file.mgcr>…        NATIVE-vs-RECORDED FIRST STATE: build the\n\
                                          level the way the port does, settle it by\n\
                                          the recorder's phase, and diff the pool,\n\
                                          every entity lane, the wizard block, the\n\
                                          board, the allocator and the LCG against\n\
                                          record 0 (census only; --settle <n>)\n\
           terrain-check <file.mgcr>…     THE NAKED TRUTH: one VERDICT line per\n\
                                          take — is the port's GENERATED terrain\n\
                                          bit-identical to what retail had at\n\
                                          record 0? The port is settled by the\n\
                                          recorder's phase, READ FROM RECORD 0\n\
                                          (MC2: 100 - the human's invuln countdown;\n\
                                          MC1: continuity byte +63 - slot), unless\n\
                                          --settle is given. Exit 1 = a plane differs\n\
           verify-deltas <file.mgcr>      import state@N, tick, diff obs@N+1\n\
           replay <file.mgcr>             PURE INPUT REPLAY: seed once from the\n\
                                          first closure, free-run on the recovered\n\
                                          input stream, report (never correct)\n\
                                          divergence at every recorded boundary;\n\
                                          gaps re-anchor a fresh segment\n\
           dump-state <file.mgcr> <t> <slot>…   print raw retail fields of\n\
                                          the given slots at tick t\n\
             --port          free-run the PORT to t (anchor at the\n\
                             take's seed, or --start <t0>; --start t-1 =\n\
                             the pair-import view) and print every lane\n\
                             side by side with retail's, ≠-marked — the\n\
                             instrument that says what the PORT holds.\n\
                             A slot retail holds FREE (class byte 0) is\n\
                             RESIDUE, never graded, and its lanes are\n\
                             homed by the CLEARED class: those rows are\n\
                             marked with a middle dot instead\n\
             --at-slot <n>   sample MID-WALK: snapshot the pool as the\n\
                             tick INTO t reaches slot n, before n\n\
                             dispatches (\"what did slot A hold when\n\
                             slot B ran\")\n\
           slice <file.mgcr> --from <t0> [--to <t1>] --out <slice.mgcr>\n\
                                          cut ticks t0..=t1 into a self-\n\
                                          contained take: header + provenance\n\
                                          (capture.slice), measured terrain\n\
                                          re-based at t0, ORIGINAL tick\n\
                                          numbers kept. The dig instrument for\n\
                                          late ticks (docs/PERF-CONFORM.md)\n\
           explain <file.mgcr> <t> [<slot>…]   retail's OWN t-1 → t\n\
                                          changelog — what CHANGED, not\n\
                                          what differs: records born/\n\
                                          freed or whose class/life-sign/\n\
                                          f70/owner moved, in full; named\n\
                                          slots always print, plus every\n\
                                          record they point at (f146/f52/\n\
                                          f54/f144/f42/f38/f40, mail\n\
                                          sources); wizard + global deltas\n\
           extract <file.mgcr> --out <manifest.json>   lift a fixture-suite\n\
                                          manifest (docs/CONFORMANCE.md)\n\
           fixtures <manifest.json>…      run a fixture suite, enforcing\n\
                                          expected statuses\n\
         \n\
         common flags:\n\
           --max-diffs <n>   mismatch paths printed per tick (default 8)\n\
           --limit <n>       stop after n tick records / pairs (default: all)\n\
         terrain-diff flags:\n\
           --baked <dir>     baked tree root (default: baked)\n\
           --out <dir>       dump both sides of every plane as raw 256x256\n\
                             byte images (<plane>.retail / <plane>.port) for\n\
                             offline clustering\n\
           --baseline <dir>  read the MEASURED planes from an earlier --out\n\
                             dump instead of this take's record-0 base (keeps\n\
                             an attribution reproducible after a re-record)\n\
           --settle <n>      tick the port's world n times (idle pose at the\n\
                             level start) before diffing, so the AUTHORED\n\
                             terrain (buildings, cave sculptors, rivers) is\n\
                             built. terrain-diff defaults to 0 (the raw\n\
                             generator), terrain-check to the recorder's phase\n\
                             (MC2 record 0 = 6 ticks in); retail@t for any\n\
                             other t is `slice --from t --to t+6`\n\
         extract flags:\n\
           --out <path>          manifest destination (required)\n\
           --sample-every <n>    conforming-pair sampling stride (default 10).\n\
                                 Failing pairs are PRINTED for the ledger,\n\
                                 never written as fixtures — a fixture\n\
                                 asserts fixed work (CONFORMANCE.md)\n\
         verify-deltas flags:\n\
           --baked <dir>     baked tree root (default: baked)\n\
           --pin-pose n|n1   drive the human with the pre- or post-tick\n\
                             recorded pose (default n1, the app's phase)\n\
           --dump <t>        print the full diff of pair t→t+1\n\
           --dump-first      print the first divergent pair in full\n\
           --csv <path>      write every per-pair diff as a TSV row\n\
                             (t, kind, slot, class, model, field, want,\n\
                             got, x, y, z, rule — for offline triage)\n\
           --no-roster       skip conformance/known-deviations.json\n\
           --no-pose-alt     skip the pose-phase pass (each dirty pair\n\
                             re-runs under the other --pin-pose sample;\n\
                             rows clean there tag `pose-phase` — retail's\n\
                             within-tick pose is two-valued and the\n\
                             capture holds one sample)\n\
           --no-slot-desync  skip the slot-desync pass (balanced same-\n\
                             (class,model) missing/extra within a pair =\n\
                             free-list slot-order desync at mass-spawn\n\
                             ticks; ledger session-4 + open-leads 0b)\n\
           --no-terrain      ignore the recording's measured terrain\n\
                             channel (format 2) — every pair runs on\n\
                             pristine planes (the A/B for the terrain\n\
                             installation)\n\
           --no-pose-lane    skip the POSE CHANNEL (the shadow mover\n\
                             step verifying the human's own motion\n\
                             column — flight state seeded from N,\n\
                             input recovered from the recorded flight\n\
                             column, pose diffed at N+1 bit-exact)\n\
                             (raw, unclassified report — docs/CONFORMANCE.md)\n\
         replay flags:\n\
           --pose-only       tier-2 chain: the FLIGHT state chains while\n\
                             the world context re-imports per pair —\n\
                             isolates mover + input recovery from world\n\
                             fidelity; world-driven pose domains reseed\n\
                             silently and are counted as gates\n\
           --segmented       SEGMENTED free run: re-anchor the free\n\
                             state from the recording at every true\n\
                             deviation (the way a capture gap already\n\
                             does), so the take reads as maximal\n\
                             continuous segments instead of one horizon\n\
                             plus noise. Certification is ONE segment\n\
                             end to end; the number that matters is\n\
                             resets in EXCESS of the gap-forced ones,\n\
                             and every reset tick names itself as a\n\
                             fixture candidate\n\
           --classify        (--segmented) run the PAIR at every\n\
                             reset-cluster head and tag it: pair DIRTY\n\
                             at t-1 ⇒ LOCAL (fixture candidate), pair\n\
                             CLEAN ⇒ INHERITED (the one-tick law is\n\
                             right — the break rides earlier state:\n\
                             unit test / upstream dig)\n\
           --stop-at-divergence\n\
                             HORIZON QUERY: stop at the first divergent\n\
                             boundary instead of reading the rest of the\n\
                             take. The horizon, the first-divergence\n\
                             render and the --brief signature are the\n\
                             same as a full run's; every COUNT is\n\
                             truncated, so both reports say stopped=<t>.\n\
                             Refused with --segmented\n\
           --brief           one machine-readable line per take\n\
                             (horizon / segments / first divergence /\n\
                             signature) — the corpus regression sweep,\n\
                             diffable against a saved baseline\n\
           --resync-deviations\n\
                             consult conformance/known-deviations.json and\n\
                             RE-ANCHOR at a boundary whose every row is a\n\
                             registered DEVIATION, instead of ending the\n\
                             run there. `replay` classifies those rows by\n\
                             default (they read `roster-excused` and are\n\
                             kept out of the excess-reset count); this flag\n\
                             is what lets a PLAIN free run keep measuring\n\
                             past one. The resync is reported, never\n\
                             hidden: the deviating value propagates, so\n\
                             the run past it is retail's state, not the\n\
                             port's own\n\
           --resync-restarts\n\
                             RE-ANCHOR at the PERMADEATH reload seam in a\n\
                             plain free run. Retail's reload re-reads the\n\
                             level from inputs the capture does not hold\n\
                             (MC1 over its own heap residue, RNG running\n\
                             mid-stream; MC2 from a pre-capture disk\n\
                             checkpoint), so native init can never land\n\
                             there. --segmented already re-anchors at the\n\
                             seam; this is what lets a PLAIN run cross one.\n\
                             Reported as resync=<n>, never hidden — the run\n\
                             past a seam is retail's state, not the port's\n\
           --no-roster       replay/verify-deltas: skip the roster\n\
                             entirely (raw, unclassified)\n\
           --start <t>       anchor the replay at tick t instead of the\n\
                             first record (records before t are skipped\n\
                             without decoding; verify-deltas warms its\n\
                             input chain over the 16 records before t)"
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
    /// terrain-diff: take the MEASURED planes from a cached `--out`
    /// dump directory instead of the recording's record-0 base.
    pub baseline: Option<PathBuf>,
    /// terrain-diff: tick the port's world this many times before
    /// taking its planes (MC2 has no load-time pass — the authored
    /// terrain lands over the first ~40 gameplay ticks).
    pub settle: Option<u32>,
    pub sample_every: u64,
    /// Feed the input channel k ticks late (retail's mouse→control→
    /// consume pipeline shows ~2-3 ticks of latency vs the sampled
    /// externals).
    pub input_delay: u64,
    /// verify-deltas: skip pairs before this tick (windowed triage;
    /// executed pairs are announced on stderr so an aborting pair
    /// self-incriminates).
    pub start: Option<u64>,
    /// slice: the tick range to cut, inclusive (`--to` absent = to the
    /// end of the take).
    pub from: Option<u64>,
    pub to: Option<u64>,
    /// Skip the known-deviation roster (raw, unclassified report).
    pub no_roster: bool,
    pub no_pose_alt: bool,
    /// Skip the computed slot-desync pass (balanced same-(class,model)
    /// missing/extra = free-list slot-order desync).
    pub no_slot_desync: bool,
    /// verify-deltas: ignore the recording's measured terrain channel
    /// and run every pair on pristine planes (the A/B for the format-2
    /// terrain installation).
    pub no_terrain: bool,
    /// Skip the pose channel (the shadow mover step over the human's
    /// own motion column).
    pub no_pose_lane: bool,
    /// replay: tier-2 chain (flight chained, world re-imported per
    /// pair) instead of the full free-running world.
    pub pose_only: bool,
    /// replay: SEGMENTED free run — re-anchor the free state from the
    /// recording at every true deviation instead of running wild after
    /// the first one, so the take reads as maximal continuous segments.
    /// Certification is ONE segment end to end; the number that matters
    /// is resets in EXCESS of the gap-forced ones.
    pub segmented: bool,
    /// replay --segmented: run the PAIR at every reset-cluster
    /// head and tag it LOCAL (pair dirty ⇒ fixture candidate) or
    /// INHERITED (pair clean ⇒ the break rides earlier state — unit
    /// test / upstream dig). The segmented-residue doctrine, automated.
    pub classify: bool,
    /// replay: one machine-readable summary line per take instead of
    /// the segment report — the whole-corpus regression sweep.
    pub brief: bool,
    /// dump-state --port: sample MID-WALK — snapshot the pool as the
    /// tick into `t` reaches this slot, before it dispatches.
    pub at_slot: Option<u16>,
    /// replay: stop the run at the FIRST divergent boundary — the
    /// horizon query, which is the single most-run question in the
    /// campaign and today pays a whole-take read to answer. The
    /// horizon, its first-divergence render and its `--brief`
    /// signature are exactly what an un-truncated run reports; every
    /// COUNT (graded/clean/segments/…) is truncated by construction,
    /// so both reports carry a `stopped=<t>` marker and no baseline
    /// can silently absorb one. Refused with `--segmented`, whose
    /// whole job is to keep measuring past the first break.
    pub stop_at_div: bool,
    /// replay: RE-ANCHOR at a roster-EXCUSED boundary in a plain free
    /// run — the instrument half of the replay-consults-known-deviations
    /// ruling (docs/CONFORMANCE.md "The known-deviation roster in
    /// `replay`"). `--segmented` already re-anchors at every break, so
    /// it needs no flag: there the roster only re-CLASSIFIES the reset.
    /// A free run cannot excuse a registered row without resyncing,
    /// because the deviating VALUE propagates (RNG draws, painted
    /// terrain) — so this re-imports retail's state at that boundary
    /// and says so, rather than pretending the run stayed bit-exact.
    pub resync_deviations: bool,
    /// replay: RE-ANCHOR at the PERMADEATH reload seam in a plain free
    /// run (player-ruled 2026-09-07). `--segmented` already does this
    /// unconditionally — `SegOpen::Restart` — because retail's reload
    /// runs machinery whose inputs are not in the capture: MC1
    /// re-reads the level over its own live heap residue with the RNG
    /// continuing MID-STREAM (there is no `srand` in the binary), MC2
    /// restores a PRE-CAPTURE disk checkpoint. Neither is reachable
    /// from native level init, so the only sound way to keep
    /// measuring past a permadeath is to re-import retail's own
    /// closure.
    ///
    /// ⚠ It is a FLAG, not the default, for one reason: a plain free
    /// run's horizon is the honest number, and a WILD post-horizon
    /// port can trip `take_restart` on state retail never held. Under
    /// this flag the run past a resync is retail's state, not the
    /// port's — the report says so.
    pub resync_restarts: bool,
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
        baseline: None,
        settle: None,
        sample_every: 10,
        no_roster: false,
        resync_deviations: false,
        resync_restarts: false,
        no_pose_alt: false,
        no_slot_desync: false,
        no_terrain: false,
        no_pose_lane: false,
        pose_only: false,
        segmented: false,
        classify: false,
        brief: false,
        at_slot: None,
        stop_at_div: false,
        input_delay: 0,
        start: None,
        from: None,
        to: None,
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
            "--no-roster" => a.no_roster = true,
            "--no-pose-alt" => a.no_pose_alt = true,
            "--no-slot-desync" => a.no_slot_desync = true,
            "--no-terrain" => a.no_terrain = true,
            "--no-pose-lane" => a.no_pose_lane = true,
            "--pose-only" => a.pose_only = true,
            "--segmented" => a.segmented = true,
            "--stop-at-divergence" => a.stop_at_div = true,
            "--resync-deviations" => a.resync_deviations = true,
            "--resync-restarts" => a.resync_restarts = true,
            "--classify" => a.classify = true,
            "--brief" => a.brief = true,
            "--port" => a.dump_port = true,
            "--at-slot" => {
                a.at_slot = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--out" => a.out = Some(it.next().map(PathBuf::from).unwrap_or_else(|| usage())),
            "--baseline" => {
                a.baseline = Some(it.next().map(PathBuf::from).unwrap_or_else(|| usage()))
            }
            "--settle" => {
                a.settle = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--sample-every" => {
                a.sample_every = it
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
            "--from" => {
                a.from = Some(
                    it.next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| usage()),
                )
            }
            "--to" => {
                a.to = Some(
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

/// A targeted read deeper than this into a FULL take should be run on
/// a slice instead (docs/PERF-CONFORM.md): every tick before the target
/// is inflated on every call, and the campaign's digs make dozens.
pub(crate) const LATE_TICK: u64 = 2000;

/// The stderr hint for a deep targeted read on a full take — silent on
/// a slice (`capture.slice`) and below [`LATE_TICK`].
pub(crate) fn late_tick_hint(rec: &Recording, path: &std::path::Path, t: u64) {
    if t <= LATE_TICK || rec.slice_provenance().is_some() {
        return;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("take");
    let from = t.saturating_sub(200);
    eprintln!(
        "hint: t={t} is deep into a full take; for a dig, cut a slice from the free-run \
         horizon and read that instead: mgc-conform slice {} --from {from} --to {} \
         --out $TMPDIR/{stem}-{from}-{}.mgcr  (docs/PERF-CONFORM.md)",
        path.display(),
        from + LATE_TICK,
        from + LATE_TICK,
    );
}

/// The stdout banner every SEEDING instrument prints for a slice.
pub(crate) fn slice_banner(rec: &Recording) {
    if let Some(p) = rec.slice_provenance() {
        println!(
            "   ⚠ {p} — seeded at its first record; a divergence born before it is \
             invisible here. Wider context = cut a wider slice."
        );
    }
}

fn main() {
    let args = parse_args();
    // `--out` belongs to terrain-diff (plane dumps) and extract (the
    // manifest destination). Every other mode ignores `args.out`, and
    // silently accepting it reads as "the tool stopped writing my
    // file" — the classic slip is `verify-deltas --out x.tsv` for
    // what is spelled `--csv x.tsv`.
    if args.out.is_some()
        && !matches!(
            args.mode.as_str(),
            "terrain-diff" | "terrain-check" | "extract" | "slice"
        )
    {
        eprintln!(
            "error: --out is not a {} flag (terrain-diff/extract/slice only); \
             the verify-deltas per-pair TSV is written with --csv <path>",
            args.mode
        );
        std::process::exit(2);
    }
    let code = match args.mode.as_str() {
        "check-decode" => args
            .files
            .iter()
            .map(|f| check_decode(f, &args))
            .max()
            .unwrap_or(0),
        "blob-census" => args
            .files
            .iter()
            .map(|f| blob_census::blob_census(f, args.limit.map(|n| n as usize)))
            .max()
            .unwrap_or(0),
        "verify-deltas" => args
            .files
            .iter()
            .map(|f| verify::verify_deltas(f, &args))
            .max()
            .unwrap_or(0),
        "replay" => args
            .files
            .iter()
            .map(|f| replay::replay(f, &args))
            .max()
            .unwrap_or(0),
        "dump-state" => dump_state(&args),
        "slice" => slice::slice(&args),
        "explain" => explain::explain(&args),
        "ground-audit" => ground_audit(&args),
        "trace" => trace(&args),
        "terrain-check" => args
            .files
            .iter()
            .map(|f| terrain_check(f, &args))
            .max()
            .unwrap_or(0),
        "terrain-diff" => args
            .files
            .iter()
            .map(|f| terrain_diff(f, &args))
            .max()
            .unwrap_or(0),
        "init-check" => args
            .files
            .iter()
            .map(|f| init_check::init_check(f, &args))
            .max()
            .unwrap_or(0),
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
    let wiz = rest.iter().any(|p| p.to_str() == Some("wiz"));
    let mut it = rest.iter().filter_map(|p| p.to_str()?.parse::<u64>().ok());
    let Some(t) = it.next() else { usage() };
    let slots: Vec<u64> = it.collect();
    if slots.is_empty() && !all && !wiz {
        usage();
    }
    // `--port`: the PORT-side dump — free-run the world to t (see
    // replay::port_dump) and print every lane of the requested
    // slots side by side with retail's, ≠-marked. The whole point is
    // that every other instrument COMPARES projections or reads the
    // recording; this one says what the port itself holds.
    if args.dump_port {
        if slots.is_empty() {
            eprintln!("dump-state --port wants explicit slot numbers");
            return 2;
        }
        let slots: Vec<u16> = slots.iter().map(|&s| s as u16).collect();
        return replay::port_dump(path, t, &slots, args);
    }
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let mc2 = rec.header.family() == Ok(mgc_formats::mgcr::Family::Mc2);
    // Nothing before `t` is read here, so skip to it without decoding
    // (the reader's byte-select path; the loop below is unchanged).
    late_tick_hint(&rec, path, t);
    if let Err(e) = rec.skip_to(t, None) {
        eprintln!("{}: {e}", path.display());
        return 2;
    }
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
            // Pop order: MC2 pops the FREE stack first and sacrifices a
            // recycle victim only when it is dry (`NewEvent_4A050`) —
            // print both tails, next-pop last.
            println!(
                "t={t} free_stack len {} tail {:?}  recycle_stack len {} tail {:?}",
                st.free_stack.len(),
                &st.free_stack[st.free_stack.len().saturating_sub(12)..],
                st.recycle_stack.len(),
                &st.recycle_stack[st.recycle_stack.len().saturating_sub(12)..],
            );
            // The per-player fleet register (`array_0x3C_60`) beside
            // the castle index it belongs to — the MC2 twin of MC1's
            // `breg=` line. INDEX ORDER IS THE LAW (the sphere pick
            // hands out targets in it), and it is not recoverable
            // from a pool census, so this is the only way to read it.
            for (pi, p) in st.players.iter().enumerate() {
                if p.play_index == 0 {
                    continue;
                }
                // The brain half beside the fleet register — the MC2
                // twin of the MC1 wizard line below. `wiz_shadow_mc2`
                // grades these lanes now, so reading them absolutely
                // (not just as mismatch rows) is what a rival-AI dig
                // needs: a state byte and a cooldown say WHICH handler
                // ran and WHICH spell it just armed.
                let cds: Vec<(usize, u16)> = p
                    .cooldown
                    .iter()
                    .enumerate()
                    .filter(|&(_, &c)| c != 0)
                    .map(|(s, &c)| (s, c))
                    .collect();
                println!(
                    "t={t} player {pi} carpet={} castle={} breg={:?} \
                     life_scale={} agg/per/refl={}/{}/{} ai_state={} \
                     burst={} charge={} pov={} invuln={} cmd_speed={} strafe={} \
                     brake={} weave={}/{} avoid={}/{} cd={cds:?}",
                    p.play_index,
                    p.castle_ent,
                    p.balloons,
                    // `word_0x24A_586` — the AUTHORED life handicap,
                    // permanently discarded to 256 on the rival's
                    // first death (round 131 W8). The only place a
                    // take shows what the `.mgcl` seeded.
                    p.life_scale,
                    p.aggression,
                    p.perception,
                    p.reflexes,
                    p.ai_state,
                    p.burst,
                    p.charge,
                    p.poverty,
                    p.invuln,
                    p.cmd_speed,
                    p.strafe,
                    p.brake,
                    p.weave_dir,
                    p.weave,
                    p.avoid,
                    p.avoid_exit,
                );
            }
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
        // Pop order: retail pops the recycle stack first, each stack
        // from its END — print both tails, next-pop last.
        println!(
            "t={t} free_stack len {} tail {:?}  recycle_stack len {} tail {:?}  erupting={} plume={}",
            st.free_stack.len(),
            &st.free_stack[st.free_stack.len().saturating_sub(12)..],
            st.recycle_stack.len(),
            &st.recycle_stack[st.recycle_stack.len().saturating_sub(12)..],
            st.erupting,
            st.plume,
        );
        if wiz || all {
            for (i, w) in st.wizards.iter().enumerate() {
                if w.play_index == 0 {
                    continue;
                }
                let cools: Vec<(usize, u16)> = w
                    .cooldown
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c != 0)
                    .map(|(s, c)| (s, *c))
                    .collect();
                let owned: Vec<(usize, u16)> = w
                    .owned_slots
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| **m != 0)
                    .map(|(s, m)| (s, *m))
                    .collect();
                let learn: Vec<(usize, u16)> = w
                    .learn
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c != 0)
                    .map(|(s, c)| (s, *c))
                    .collect();
                // Verbatim, position included — the list is a
                // phase-tagged union (alive slots / dead models with
                // −1 empties) and INDEX is meaningful to the scatter
                // and the respawn regrant, so no zero-filtering.
                let acq: Vec<i32> = w.spell_list.to_vec();
                println!(
                    "t={t} wiz {i}: ent={} mv={:#x} hands=({},{}) charge={} \
                     grace={} stall={} rate={} ai_state={} burst={} pov={} \
                     castle={} aggro={} tempo={} breg={:?} owned={owned:?} \
                     cool={cools:?} learn={learn:?} acq={acq:?}",
                    w.play_index,
                    w.move_bits,
                    w.hand_left,
                    w.hand_right,
                    w.charge,
                    w.grace,
                    w.regen_stall,
                    w.life_rate,
                    w.ai_state,
                    w.burst,
                    w.poverty,
                    w.castle,
                    w.aggro,
                    w.tempo,
                    w.balloon_reg,
                );
            }
        }
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

/// Compare retail entities' rest-z against the port's generated
/// ground plane at their coordinates (`ground-audit <file.mgcr>
/// [--dump <t>]`). At t=0 no runtime terrain edit exists yet, so the
/// grounded statics (trees, standing fires, huts) sample retail's
/// PRISTINE plane — a generator-fidelity probe that needs no live
/// DOSBox height dump. Late ticks measure edits + shortfall mixed.
fn ground_audit(args: &Args) -> i32 {
    let Some(path) = args.files.first() else {
        usage()
    };
    let t = args.dump.unwrap_or(0);
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return 2;
        }
    };
    let game = rec.header.game.clone();
    let Some(level) = rec.header.level else {
        eprintln!("recording has no level number");
        return 2;
    };
    if rec.header.family() != Ok(mgc_formats::mgcr::Family::Mc1) {
        eprintln!("ground-audit is MC1/HW-only (class-2 snap law)");
        return 2;
    }
    let (world, _) = match verify::build_world(&args.baked, &game, level) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("build: {e}");
            return 2;
        }
    };
    if let Err(e) = rec.skip_to(t, None) {
        eprintln!("{}: {e}", path.display());
        return 2;
    }
    while let Some(r) = rec.next_tick() {
        let tick = match r {
            Ok(x) => x,
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
        let st = match mgc_formats::mgcr::decode_retail_mc1(state) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("t={t}: {e}");
                return 2;
            }
        };
        // Residual histogram per (class, model), plus per-site rows
        // for anything off by a height byte or more (|dz| >= 32).
        use std::collections::BTreeMap;
        let mut fam: BTreeMap<(u8, u8), (u64, i64, i64)> = BTreeMap::new();
        let mut sites: BTreeMap<(u16, u16), (u64, i64)> = BTreeMap::new();
        let grounded = |e: &mgc_formats::mgcr::RetailEntMc1| {
            e.class64 == 2
                || (e.class64 == 10 && matches!(e.model65, 0 | 45))
                || (e.class64 == 3 && e.model65 == 2)
                || e.class64 == 5
        };
        for e in st.ents.iter().filter(|e| e.class64 != 0) {
            if !grounded(e) {
                continue;
            }
            let gz = world.ground_z_engine(e.x, e.y);
            let dz = e.z as i64 - gz as i64;
            let f = fam.entry((e.class64, e.model65)).or_default();
            f.0 += 1;
            f.1 += dz;
            f.2 = f.2.max(dz.abs());
            if dz.abs() >= 32 {
                let s = sites.entry((e.x >> 8 & !15, e.y >> 8 & !15)).or_default();
                s.0 += 1;
                s.1 += dz;
            }
        }
        println!("== ground-audit {} t={t}", path.display());
        for ((c, m), (n, sum, max)) in &fam {
            println!(
                "  ({c},{m}): {n} sampled, mean dz {:+.1}, max |dz| {max}",
                *sum as f64 / *n as f64
            );
        }
        println!("  sites with |dz| >= 32 (16-tile grid, count, mean dz):");
        let mut rows: Vec<_> = sites.into_iter().collect();
        rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        for ((sx, sy), (n, sum)) in rows.into_iter().take(24) {
            println!("    ({sx},{sy}): {n}  mean {:+.1}", sum as f64 / n as f64);
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
    if rec.skip_to(t0, None).is_err() {
        return 2;
    }
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

/// The recorder's record-0 PHASE: how many gameplay ticks retail had
/// already run when the take's first terrain base was captured — READ
/// FROM THE TAKE (round 110). It is per-take, not per-game: the
/// recorder attaches some ticks after LoadLevel, and round 109's
/// constant "MC2 = 6 ticks" was only mc2l15's fit (mc2l3/mc2l30 are 8
/// in, mc2l22 9; MC1 takes sit 8..24 ticks in — its villager craters
/// and the rival's starting castle were "missing" for exactly that
/// reason).
/// - MC2: the human's spawn-invulnerability countdown
///   (`word_0x159_345`, `RetailPlayerMc2::invuln`) starts at 100
///   (`Rival::grace`) and steps −1 per tick, so phase = 100 − invuln
///   (mc2l15: 94 → 6, the round-109 unique fit; mc2l30: 92 → 8 =
///   IDENTICAL on all five planes).
/// - MC1: the per-entity continuity byte `+63` is seeded with the
///   slot index at spawn (`Gen::spawn`, `e.f63 = idx`) and steps +1
///   per dispatched tick, so phase = f63 − slot over the first live
///   slots; the MAX wins because a slot the dispatcher skipped only
///   reads LOWER (mc1l49: slots 1..3 = 18/18/21 → 18, the settle at
///   which its ten craters match). A slot whose `+63` reads BELOW its
///   own index is not a settle clock at all — round 155: mc1l34/l36
///   seat class-5 creatures in slots 3..8 whose `+63` sits at
///   `slot + phase − 5`, and the wrapped 254/255 won the max, the LCG
///   ±2 search could not recover, and both takes came in as
///   thousands of DIFFERENT terrain cells. Those reads are dropped.
///   ⚠ And the class-5 `+63` is not a slot clock at all (w155e): every
///   MC1 creature ctor stamps `+63 = spawn_count[model]++`, the PER-MODEL
///   spawn ordinal (sub_main.cpp:44750-52 for m2, `*(v8 + 12)` read then
///   `+ 1`, the same shape at :44623/:44698/:44930/:45000/:45282/…; the
///   port's `mobs.rs` ctor `e.f63 = ordinal`), so "`slot + phase − 5`"
///   is only the first `(5,2)` sitting in slot 5. A creature whose
///   ordinal + phase happens to reach its slot would pass the `>= s`
///   filter and read a LOW phase; class 5 is skipped outright.
///
/// `None` when the record has no decodable state (older takes).
fn retail_record0_phase(
    first: &mgc_formats::mgcr::TickRecord,
    family: mgc_formats::mgcr::Family,
) -> Option<u32> {
    let state = first.state.as_ref()?;
    match family {
        mgc_formats::mgcr::Family::Mc1 => {
            let st = mgc_formats::mgcr::decode_retail_mc1(state).ok()?;
            // Creatures (class 5) are the fallback only: their `+63` is a
            // spawn ordinal + phase, so the slot read is approximate
            // (mc1l11/l16/l42 seat nothing else in 1..8 and read one low;
            // `record0_settle`'s LCG search corrects it).
            let read = |creatures: bool| {
                (1..=8usize)
                    .filter_map(|s| {
                        let e = st.ents.get(s)?;
                        (e.class64 != 0 && (e.class64 == 5) == creatures && e.f63 as usize >= s)
                            .then(|| (e.f63 as usize - s) as u32)
                    })
                    .max()
            };
            read(false).or_else(|| read(true))
        }
        mgc_formats::mgcr::Family::Mc2 => {
            let st = mgc_formats::mgcr::decode_retail_mc2(state).ok()?;
            let p = st
                .players
                .get(st.local_player as usize)
                .or_else(|| st.players.first())?;
            Some((100 - p.invuln as i32).clamp(0, 255) as u32)
        }
    }
}

/// The settle count `init-check`/`terrain-check` use when `--settle` is
/// not given: the record-0 phase, CORRECTED BY THE LCG. Round 153: on
/// four of 39 MC1 takes (mc1l11/l16/l42 — and mc1l6, which no count
/// fits) the `+63 − slot` phase read one tick LOW — every level record
/// then sat one tick young (1,913 lanes rows on mc1l11, `rand` DIFF)
/// and the phase+1 build was IDENTICAL on the LCG with 113 rows. The
/// global LCG is the sharper clock: one draw per dispatched record per
/// tick, so a settle count off by one can never match it. The phase
/// stays the first candidate (a clean take never re-settles); ±1, then
/// ±2 are tried only when it disagrees; no fit falls back to the phase.
/// Returns `(settle, corrected)`.
pub(crate) fn record0_settle(
    path: &std::path::Path,
    first: &mgc_formats::mgcr::TickRecord,
    family: mgc_formats::mgcr::Family,
    game: &str,
    level: u32,
    args: &Args,
) -> Result<(u32, bool), String> {
    let phase = retail_record0_phase(first, family)
        .ok_or("record 0 carries no decodable state to read the phase from — pass --settle <n>")?;
    let state = first.state.as_ref().ok_or("record 0 carries no state channel")?;
    let want = match family {
        mgc_formats::mgcr::Family::Mc1 => mgc_formats::mgcr::decode_retail_mc1(state)?.rand,
        mgc_formats::mgcr::Family::Mc2 => mgc_formats::mgcr::decode_retail_mc2(state)?.rand,
    };
    let fits = |n: u32| -> bool {
        native_settled_world(path, first, family, game, level, args, n)
            .map(|(w, _)| w.rand_state() == want)
            .unwrap_or(false)
    };
    if fits(phase) {
        return Ok((phase, false));
    }
    for d in [1i64, -1, 2, -2] {
        let n = phase as i64 + d;
        if n >= 0 && fits(n as u32) {
            return Ok((n as u32, true));
        }
    }
    Ok((phase, false))
}

/// The human's level-start book as retail's record 0 holds it: every
/// class-15 token whose `parentId` (@0x28) is the local player's carpet
/// slot, in slot order. `None` when record 0 carries no decodable MC2
/// state or no carpet.
fn retail_record0_human_book(first: &mgc_formats::mgcr::TickRecord) -> Option<Vec<u8>> {
    let state = first.state.as_ref()?;
    let st = mgc_formats::mgcr::decode_retail_mc2(state).ok()?;
    let p = st
        .players
        .get(st.local_player as usize)
        .or_else(|| st.players.first())?;
    let carpet = p.play_index;
    if carpet == 0 {
        return None;
    }
    Some(
        st.ents
            .iter()
            .filter(|e| e.class3f == 15 && e.owner28 == carpet)
            .map(|e| e.model40)
            .collect(),
    )
}

/// The MC1 twin of [`retail_record0_human_book`]: the human's carried
/// spells at record 0 in ACQUISITION order — the wizard's `+532` list
/// holds the tokens' pool slots in pickup order while he is alive
/// (`sub_3DD50` :49240-58 fills it in `byte_99B88` order and
/// `sub_44D30` :54882-905 mints one token per entry, rewriting each
/// entry to the slot), and a class-12 token's model IS its spell id.
/// An EMPTY book is still `Some` (mc1l37/mc1hwl0 start bookless and
/// the carpet record must be seated all the same); `None` only when
/// record 0 carries no seated human — the caller then builds the app's
/// own layout.
fn retail_record0_human_book_mc1(first: &mgc_formats::mgcr::TickRecord) -> Option<Vec<u8>> {
    let state = first.state.as_ref()?;
    let st = mgc_formats::mgcr::decode_retail_mc1(state).ok()?;
    let w = st
        .wizards
        .get(st.local_player as usize)
        .or_else(|| st.wizards.first())?;
    if w.play_index == 0 {
        return None;
    }
    let book: Vec<u8> = w
        .spell_list
        .iter()
        .take_while(|&&s| s > 0)
        .filter_map(|&s| {
            let e = st.ents.get(s as usize)?;
            (e.class64 == 12 && e.f42 == w.play_index).then_some(e.model65)
        })
        .collect();
    Some(book)
}

/// The human's book PROGRESS at record 0 (`levels`, `xp_bank`, `ring`)
/// — the carried save's half of [`retail_record0_human_book`].
fn retail_record0_human_progress(
    first: &mgc_formats::mgcr::TickRecord,
) -> Option<([u8; 26], [i32; 26], [u8; 26])> {
    let st = mgc_formats::mgcr::decode_retail_mc2(first.state.as_ref()?).ok()?;
    let p = st
        .players
        .get(st.local_player as usize)
        .or_else(|| st.players.first())?;
    Some((p.levels, p.xp_bank, p.ring))
}

/// One take's generated-vs-measured terrain comparison.
struct TerrainReport {
    game: String,
    level: u32,
    base_t: u64,
    settle: u32,
    /// (plane, cells, differing cells, first examples as (x, y, retail, port))
    planes: Vec<(String, usize, usize, Vec<(usize, usize, u8, u8)>)>,
    skipped: Vec<String>,
}

impl TerrainReport {
    fn identical(&self) -> bool {
        self.planes.iter().all(|p| p.2 == 0)
    }

    /// The verdict line's honesty tail: a plane the port does not
    /// generate is NOT evidence of agreement, so an IDENTICAL verdict
    /// has to say which lanes it never looked at.
    fn ungraded_tail(&self) -> String {
        if self.skipped.is_empty() {
            String::new()
        } else {
            format!("  ⚠ UNGRADED: {}", self.skipped.join(", "))
        }
    }
}

/// THE NATIVE WORLD AT RECORD 0 — the port's own level build
/// (`verify::build_world` / `build_world_mc2_with_book`, the human's
/// book read off the take), ticked `settle` times with the carpet
/// idle at the authored start. Shared by `terrain-check` (which keeps
/// only the planes) and `init-check` (which diffs the whole world
/// against the take's first closure), so the two instruments can
/// never settle differently.
pub(crate) fn native_settled_world(
    path: &std::path::Path,
    first: &mgc_formats::mgcr::TickRecord,
    family: mgc_formats::mgcr::Family,
    game: &str,
    level: u32,
    args: &Args,
    settle: u32,
) -> Result<(mgc_sim::engine::world::World, mgc_sim::engine::features::Planes), String> {
    let (mut w, pristine) = match family {
        mgc_formats::mgcr::Family::Mc1 => {
            // The human's carried book off record 0, in acquisition
            // order, granted in `sub_44D30`'s order (carpet record,
            // tokens, then the rivals) — round 153's MC1 twin of the
            // MC2 arm below. The seat itself is the constructor's
            // since round 154 (`MGC_NO_MC1_NATIVE_HUMAN_RECORD=1`
            // for the A/B).
            let book = retail_record0_human_book_mc1(first);
            verify::build_world_mc1_with_book(&args.baked, game, level, book.as_deref())?
        }
        mgc_formats::mgcr::Family::Mc2 => {
            // The human's carried book, read off record 0: the class-15
            // tokens owned by the (3,0) carpet record, in slot order.
            // Granted between the ctor and the rival spawn so the
            // native pool lays out like retail's (round 112).
            let book = retail_record0_human_book(first);
            // ⭐ THE CAMPAIGN REPLAY GATE IS A LOAD-TIME INPUT
            // (`setting_38545 & 4`, MenusAndIntros.cpp:3347-48 — the
            // world map raises it for a MAIN portal this save has
            // already completed). The scroll ctor `sub_51610`
            // (EF:37416-37426) and `UpdateScroll_59C80`'s opening
            // (EF:41161-65) both read it, so a native build that
            // hard-codes `false` mints and KEEPS the level's dis-0
            // `(14,5)` XP scrolls that retail hides and reap-flags in
            // their first dispatch. `terrain-check` never noticed
            // (the gated arm writes no terrain); `init-check` sees it
            // as port-only pool rows (mc2l15: retail 0, port 3).
            let replayed = verify_mc2::mc2_take_replayed(path)?;
            let (mut w, p, _) = verify_mc2::build_world_mc2_with_book(
                &args.baked,
                level,
                replayed,
                book.as_deref(),
            )?;
            // …and the carried save's PROGRESS in it, which no native
            // build can know (round 148: the three wiz-0 lanes that
            // fired on nearly every take were this seat, not the port).
            if let Some((levels, xp_bank, ring)) = retail_record0_human_progress(first) {
                w.mc2_seed_book_progress(levels, xp_bank, ring);
            }
            (w, p)
        }
    };
    if std::env::var_os("MGC_POOL_CENSUS").is_some() {
        let rows = w.debug_pool_rows();
        let row: Vec<String> = rows
            .iter()
            .map(|(j, c, m, a, f26, f59)| format!("{j}:({c},{m})a{a}/{f26}/{f59}"))
            .collect();
        eprintln!("POOL CENSUS live={}\n{}", rows.len(), row.join(" "));
    }
    let planes = if settle > 0 {
        // The app's `--map-settle` driver: real ticks (not
        // `tick_paused`), the carpet idle at the level start.
        let (px, pz) = mc2_player_start(&args.baked, &family, level).unwrap_or((128.5, 128.5));
        let idle = mgc_sim::engine::world::PlayerCommand::default();
        let settle_alt: f32 = std::env::var("MGC_INIT_SETTLE_ALT")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(1.0);
        // ⭐ `MGC_INIT_AUTOSAVE_AT=<n>|none` — the frame retail's
        // one-shot level-start checkpoint autosave landed on FOR THIS
        // TAKE. `sub_57640`'s only caller is the level-start palette
        // fade-in, so the frame is a property of the recording session
        // (frame rate, disk), not of the level, and the corpus proves
        // the variance: mc2l30 holds a canonical free stack at record 0
        // and mc2l30-new — same level, same frame, same human record,
        // same three freed transients — holds the raw LIFO residue. See
        // `World::mc2_arm_checkpoint_autosave`. This is the SAME EVENT
        // `MGC_INIT_SEVER_AT` models through the StageVar lane; feed
        // both from one number per take.
        // Unset, the build's own `AUTOSAVE_DEFAULT_FRAME` stands.
        let mut autosave_at: Option<u8> =
            Some(mgc_sim::engine::world::World::AUTOSAVE_DEFAULT_FRAME);
        if family == mgc_formats::mgcr::Family::Mc2
            && let Ok(v) = std::env::var("MGC_INIT_AUTOSAVE_AT")
        {
            let v = v.trim();
            let n = if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("off") {
                None
            } else {
                v.parse::<u8>().ok()
            };
            w.mc2_arm_checkpoint_autosave(n);
            autosave_at = n;
        }
        // `MGC_INIT_SEVER_AT=<n>|none` — apply retail's OWN
        // autosave-severed StageVar closure after n settle ticks (the
        // registered `&2`-clear death-watch deviation; see
        // `World::mc2_debug_sever_stagevar_watches`). ⚖ Player-ruled
        // 2026-09-18 (round 149): DEFAULT ON, at the autosave frame —
        // the severance IS the checkpoint autosave's other half, so
        // one number per take drives both and a bare `init-check`
        // reads the standard recording session (38/40 MC2 takes
        // IDENTICAL). `none`/`off` restores the plain native settle.
        let sever_at: Option<u32> = match std::env::var("MGC_INIT_SEVER_AT") {
            Ok(v) => {
                let v = v.trim();
                if v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("off") {
                    None
                } else {
                    v.parse().ok()
                }
            }
            Err(_) => autosave_at.map(u32::from),
        };
        for k in 0..settle {
            if sever_at == Some(k)
                && family == mgc_formats::mgcr::Family::Mc2
                && let Some(state) = first.state.as_ref()
                && let Ok(st) = mgc_formats::mgcr::decode_retail_mc2(state)
            {
                let mask = st
                    .stagevars
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r[1] & 0x04 != 0)
                    .fold(0u16, |m, (i, _)| m | 1 << i);
                w.mc2_debug_sever_stagevar_watches(&st.stagevar_watch, mask);
            }
            // ⭐ ONE TILE UP, NOT TWO. Retail seats the carpet at the
            // marker's tile centre, `ground + 0x100` (`sub_44D30`
            // :54838-42; MC2 Level.cpp:1319 — the same snap both
            // `mc1_spawn_human_record` and `mc2_spawn_human_record`
            // take), and the settle window's idle input holds it
            // there: mc1l20 record 0 reads the (3,0) carpet at 2480 =
            // ground 2224 + 256. The app's `--map-settle` convention
            // (`+ 2.0`) stood the settle pose one tile too high, and
            // everything that reads the carpet's ALTITUDE in the
            // settle window followed it: the (5,11) genies' ambush
            // blink lands "at the target's altitude" (:24733), their
            // (10,1) sparkle ring and the (10,0) fires it sheds are
            // laid at the genie's z — round 154's "(10,0) z retail
            // 2498 vs port 2754" lead (mc1l20 55 + 24 + 3 rows,
            // mc1l16 34, all exactly +256) was this line, not a fire
            // ctor (round 154, w154k). `MGC_INIT_SETTLE_ALT=<tiles>`
            // overrides the lift for an A/B (`2.0` = the old pose).
            let alt = w.ground_height_tiles(px, pz) + settle_alt;
            let pose = mgc_sim::engine::world::PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);
            w.tick(pose, idle);
        }
        if std::env::var_os("MGC_POOL_CENSUS").is_some() {
            let rows = w.debug_pool_rows();
            let row: Vec<String> = rows
                .iter()
                .map(|(j, c, m, a, f26, f59)| format!("{j}:({c},{m})a{a}/{f26}/{f59}"))
                .collect();
            eprintln!(
                "POOL CENSUS AFTER SETTLE live={}\n{}",
                rows.len(),
                row.join(" ")
            );
        }
        w.planes_clone()
    } else {
        pristine
    };
    Ok((w, planes))
}

/// The shared core of `terrain-diff` and `terrain-check`: decode the
/// take's record-0 terrain base (or a `--baseline` dump), build the
/// port's level, settle it `settle` ticks, and count differing cells
/// per declared plane.
fn terrain_compare(
    path: &std::path::Path,
    args: &Args,
    settle: Option<u32>,
) -> Result<TerrainReport, String> {
    let mut rec = Recording::open(path)?;
    let decl = rec
        .header
        .channels
        .terrain
        .clone()
        .ok_or("recording has no terrain channel (format-1 take?)")?;
    let game = rec.header.game.clone();
    let family = rec.header.family()?;
    let level = rec.header.level.ok_or("recording has no level number")?;
    let first = rec
        .next_tick()
        .ok_or("empty recording")?
        .map_err(|e| e.to_string())?;
    let base = first
        .terrain
        .as_ref()
        .and_then(|b| b.base.clone())
        .ok_or("first record carries no terrain base")?;
    // `None` = settle by the recorder's own phase, read from record 0
    // (`retail_record0_phase`); `Some(n)` = the caller's explicit count.
    let settle = match settle {
        Some(n) => n,
        None => record0_settle(path, &first, family, &game, level, args)?.0,
    };
    let mut img = mgc_formats::mgcr::TerrainImage::new(&decl);
    img.apply(&mgc_formats::mgcr::TerrainBlock {
        base: Some(base),
        delta: None,
    })?;
    let (_, planes) = native_settled_world(path, &first, family, &game, level, args, settle)?;
    if let Some(dir) = &args.out {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut report = TerrainReport {
        game: game.to_string(),
        level,
        base_t: first.t,
        settle,
        planes: Vec::new(),
        skipped: Vec::new(),
    };
    for name in &decl.planes {
        // `--baseline <dir>`: read the MEASURED planes from a cached
        // `--out` dump (`<dir>/<plane>.retail`) instead of the take's
        // own record-0 base. A take can be lost or re-recorded; a
        // cached dump of a graded take keeps the stock-bake validator
        // reproducible against the exact planes an attribution was
        // written from.
        let cached;
        let measured: &[u8] = if let Some(dir) = args.baseline.as_ref() {
            cached = std::fs::read(dir.join(format!("{name}.retail")))
                .map_err(|e| format!("{}/{name}.retail: {e}", dir.display()))?;
            &cached
        } else {
            img.plane(name).ok_or("declared plane missing")?
        };
        let baked: &[u8] = match name.as_str() {
            "type" => &planes.tile_type,
            "height" => &planes.height,
            "shading" => &planes.shading,
            "angle" => &planes.angle,
            "ceiling" => &planes.ceiling,
            other => {
                report.skipped.push(other.to_string());
                continue;
            }
        };
        if let Some(dir) = &args.out {
            std::fs::write(dir.join(format!("{name}.retail")), measured)
                .map_err(|e| format!("{name}.retail: {e}"))?;
            std::fs::write(dir.join(format!("{name}.port")), baked)
                .map_err(|e| format!("{name}.port: {e}"))?;
        }
        // ⚠ THE RECORDER DECLARES A CEILING ON EVERY MC2 TAKE SINCE
        // e792c5b, CAVE OR NOT — and off-cave the port generates no
        // ceiling at all (`Planes::ceiling` stays empty). That is the
        // same UNGRADED LANE `replay` reports as "measured CEILING
        // dropped: this level carries no ceiling plane (off-cave)";
        // erroring the whole take out on it threw away the verdict on
        // the four planes that ARE comparable, which is how eight of
        // round 143's eleven new takes arrived with no terrain
        // verdict at all. An EMPTY port plane is a lane the port does
        // not model; any OTHER size disagreement is still a defect.
        if baked.is_empty() && !measured.is_empty() {
            report.skipped.push(format!("{name} (port generates none)"));
            continue;
        }
        if baked.len() != measured.len() {
            return Err(format!(
                "{name}: size mismatch — port {} cells vs measured {}",
                baked.len(),
                measured.len()
            ));
        }
        let mut examples = Vec::new();
        let diffs = (0..measured.len())
            .filter(|&i| measured[i] != baked[i])
            .inspect(|&i| {
                if examples.len() < args.max_diffs {
                    examples.push((i % 256, i / 256, measured[i], baked[i]));
                }
            })
            .count();
        report
            .planes
            .push((name.clone(), measured.len(), diffs, examples));
    }
    Ok(report)
}

/// The level's authored player start (the class-3 model-4 THING,
/// tile centre) — the app's `entities::player_start` resolver.
fn mc2_player_start(
    baked: &std::path::Path,
    family: &mgc_formats::mgcr::Family,
    level: u32,
) -> Option<(f32, f32)> {
    let dir = match family {
        mgc_formats::mgcr::Family::Mc1 => "mc1",
        mgc_formats::mgcr::Family::Mc2 => "mc2",
    };
    let lp = baked.join(dir).join(format!("level-{level:03}.mgcl"));
    let pkg: mgc_formats::LevelPackage =
        mgc_formats::mgcl::read(std::fs::File::open(lp).ok()?).ok()?;
    pkg.things
        .things
        .iter()
        .find(|t| t.kind == mgc_formats::ThingKind::Entity && t.class == 3 && t.model == 4)
        .map(|t| (t.x as f32 + 0.5, t.y as f32 + 0.5))
}

/// `terrain-diff <rec.mgcr>…` — the record-0 STOCK-BAKE VALIDATOR
/// (docs/RECORDING-TERRAIN-V2.md "free instruments"): decode the
/// take's measured terrain base and diff it plane-by-plane against
/// the port's own generated level terrain. Agreement certifies the
/// generator chain; disagreement prints cell-level examples to dig
/// at. Exit 0 = every compared plane matched.
fn terrain_diff(path: &std::path::Path, args: &Args) -> i32 {
    match terrain_compare(path, args, Some(args.settle.unwrap_or(0))) {
        Ok(r) => {
            println!(
                "== terrain-diff {} (game {}, level {}, base @t={}, port settle {})",
                path.display(),
                r.game,
                r.level,
                r.base_t,
                r.settle
            );
            for other in &r.skipped {
                println!("  {other}: UNGRADED — skipped");
            }
            for (name, cells, diffs, examples) in &r.planes {
                if *diffs == 0 {
                    println!("  {name}: MATCH ({cells} cells)");
                    continue;
                }
                println!(
                    "  {name}: {diffs} cell(s) differ ({:.2}%); examples:",
                    *diffs as f64 * 100.0 / *cells as f64
                );
                for (x, y, retail, port) in examples {
                    println!("    ({x:3},{y:3}) retail {retail:3} vs port {port:3}");
                }
            }
            if r.identical() { 0 } else { 1 }
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            2
        }
    }
}

/// `terrain-check <rec.mgcr>…` — THE NAKED TRUTH, one line per take:
/// is the port's GENERATED level terrain bit-identical to what retail
/// had when the take began? The port is settled by the recorder's
/// record-0 phase ([`retail_record0_phase`]) unless `--settle` says
/// otherwise, then every declared plane must match on every cell.
/// This is the GENERATE-axis witness — `replay` imports the measured
/// terrain every pair and can never see a generator deviation
/// (round 109: the mc2:15 lava was missing for 100+ rounds while the
/// take certified). Exit 0 = identical, 1 = some plane differs.
fn terrain_check(path: &std::path::Path, args: &Args) -> i32 {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    match terrain_compare(path, args, args.settle) {
        Ok(r) if r.identical() => {
            println!(
                "TERRAIN {name}: IDENTICAL — {} plane(s) × {} cells, port settled {} tick(s){}{}",
                r.planes.len(),
                r.planes.first().map_or(0, |p| p.1),
                r.settle,
                if args.settle.is_none() {
                    " (recorder phase, read from record 0)"
                } else {
                    ""
                },
                r.ungraded_tail()
            );
            0
        }
        Ok(r) => {
            let cells = r.planes.first().map_or(0, |p| p.1);
            let detail: Vec<String> = r
                .planes
                .iter()
                .filter(|p| p.2 != 0)
                .map(|p| format!("{} {}", p.0, p.2))
                .collect();
            println!(
                "TERRAIN {name}: DIFFERENT — {} of {cells} cells, port settled {} tick(s){} (level {}, \
                 base @t={}){}",
                detail.join(" · "),
                r.settle,
                if args.settle.is_none() {
                    " = recorder phase"
                } else {
                    ""
                },
                r.level,
                r.base_t,
                r.ungraded_tail()
            );
            1
        }
        Err(e) => {
            println!("TERRAIN {name}: ERROR — {e}");
            2
        }
    }
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
    // Terrain channel (format 2): every record's block must decode and
    // accumulate cleanly; report the channel's shape at the end.
    let mut timg = rec
        .header
        .channels
        .terrain
        .as_ref()
        .map(mgc_formats::mgcr::TerrainImage::new);
    let (mut t_deltas, mut t_cells) = (0u64, 0u64);
    while let Some(r) = rec.next_tick() {
        let tick = match r {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  record error: {e}");
                return 2;
            }
        };
        ticks += 1;
        if let (Some(img), Some(block)) = (timg.as_mut(), &tick.terrain) {
            if let Some(d) = &block.delta {
                match mgc_formats::mgcr::decode_terrain_delta(
                    d,
                    img.decl().planes.len(),
                    img.decl().cells(),
                ) {
                    Ok(planes) => {
                        t_deltas += 1;
                        t_cells += planes.iter().map(|p| p.len() as u64).sum::<u64>();
                    }
                    Err(e) => {
                        eprintln!("  t={}: terrain: {e}", tick.t);
                        bad += 1;
                    }
                }
            }
            if let Err(e) = img.apply(block) {
                eprintln!("  t={}: terrain: {e}", tick.t);
                bad += 1;
            }
        }
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
        // `check-decode` is the one consumer that wants the stored obs
        // as a `Value` (the strict comparator diffs value trees), so it
        // — and only it — pays for materialising one.
        let stored: serde_json::Value = match serde_json::from_str(stored.get()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  t={}: obs: {e}", tick.t);
                bad += 1;
                continue;
            }
        };
        let diffs = jsondiff::diff(&stored, &decoded, args.max_diffs);
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
    if let Some(img) = &timg {
        println!(
            "  terrain: base {}, {} delta record(s), {} cell edit(s) total",
            if img.based() { "present" } else { "MISSING" },
            t_deltas,
            t_cells
        );
        if !img.based() {
            eprintln!("  terrain channel declared but no base record seen");
            return 1;
        }
    }
    if bad == 0 { 0 } else { 1 }
}
