//! `mgc-conform slice` — cut a contiguous tick range out of a take into
//! a self-contained recording.
//!
//! THE COST IT REMOVES (docs/PERF-CONFORM.md "RECORDING SLICES"): every
//! dig instrument that reads a late tick has to inflate and walk the
//! whole take up to it, so digs slow down exactly as the campaign
//! succeeds and the free-run horizon moves out. A slice from the
//! horizon forward is small, and every read of it is cheap; the full
//! take is then only for the neutrality proof.
//!
//! WHAT A SLICE CARRIES — the same closure the full take has at those
//! ticks, so `replay`, `verify-deltas`, `dump-state`, `explain` and the
//! fixture extractor run on it unchanged:
//! - the header, verbatim, plus `capture.slice` provenance (source
//!   file, requested range, the command);
//! - the measured terrain channel RE-BASED at the first record: the
//!   planes at the cut are the sum of every delta before it, so the
//!   cutter folds them while skipping and writes the result as that
//!   record's `base_b64` (the same materialisation
//!   `recordings/cut_mgcr.py` does at a level transition);
//! - every other record byte for byte, ORIGINAL tick numbers kept, so
//!   a head at t in the slice IS the head at t in the take and no
//!   citation ever needs translating.
//!
//! ⚠ A slice is a `--start`-shaped view (the caveat in
//! PERF-CONFORM.md): a run seeded at its first record cannot see a
//! divergence born before it. Cut from the free-run horizon, not from
//! the head — and if a dig needs earlier context, cut a wider one.
//!
//! ⛔ **"THE RE-BASE PUTS THE SLICE ON A DIFFERENT TERRAIN BASELINE" —
//! REFUTED, round 149.** The worry was that a take whose terrain is
//! MEASURED from record 0 would replay a slice against record 0's
//! planes, so a terrain-gated head could appear or vanish. It cannot:
//! [`TerrainImage::base_blob`] returns `self.planes.concat()`, the
//! LIVE accumulated image — every delta up to AND INCLUDING the cut
//! record is folded in above before it is read — so the slice's first
//! record carries exactly the planes the full take's reader holds at
//! that tick. If no base has been seen the cutter REFUSES rather than
//! writing an unanchored blob.
//!
//! Measured, not asserted (mc2l24, certified, `--from 30000 --to
//! 32000`): `dump-state --port 32000 1 2 3 4 5 6 --start 30000` on the
//! SLICE and on the FULL TAKE are byte-identical over 557 lines — a
//! 2,000-tick free run, so any plane difference at the anchor would
//! have shown — and `replay --segmented --brief` on the slice reads
//! `devs=0 graded=2000 clean=2000`.
//!
//! What a slice really changes is the **ANCHOR**, not the terrain: the
//! port free-runs from retail's state at `t0` instead of from its own
//! accumulated state there, and `--segmented` has fewer re-anchors to
//! spend. Same take, same window, measured: whole-take `mc2l24` is
//! `devs=0 horizon=END`, while `--start 30000` is `devs=2
//! horizon=35680`. Attribute that to the anchor, never to the planes.
use crate::Args;
use mgc_formats::mgcr::{Recording, RecordingWriter, TerrainImage, TickRecord, b64_encode};

/// Temporary slices are read a few times and thrown away; level 9 on a
/// gigabyte of inflated JSON would be most of the cut.
const SLICE_ZSTD_LEVEL: i32 = 3;

pub fn slice(args: &Args) -> i32 {
    match run(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("slice: {e}");
            2
        }
    }
}

fn run(args: &Args) -> Result<(), String> {
    let path = args.files.first().ok_or("slice wants <take.mgcr>")?;
    let from = args.from.ok_or("slice wants --from <t0>")?;
    let to = args.to.unwrap_or(u64::MAX);
    if to < from {
        return Err(format!("--to {to} is below --from {from}"));
    }
    let out = args.out.as_ref().ok_or("slice wants --out <slice.mgcr>")?;
    if out == path {
        return Err("--out must not be the source take".to_string());
    }
    let mut rec = Recording::open(path)?;
    let mut timg = rec.header.channels.terrain.as_ref().map(TerrainImage::new);
    if !rec.skip_to(from, timg.as_mut())? {
        return Err(format!(
            "{}: no record at or after t={from}",
            path.display()
        ));
    }

    // Header: verbatim, plus provenance.
    let src_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string();
    let mut header = rec.header_json.clone();
    let hobj = header
        .as_object_mut()
        .ok_or("header record is not a JSON object")?;
    let cap = hobj
        .entry("capture")
        .or_insert_with(|| serde_json::json!({}));
    if !cap.is_object() {
        *cap = serde_json::json!({});
    }
    let cmd = format!(
        "mgc-conform slice {src_name} --from {from}{} --out {}",
        args.to.map(|t| format!(" --to {t}")).unwrap_or_default(),
        out.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
    );
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    cap.as_object_mut().expect("object").insert(
        "slice".to_string(),
        serde_json::json!({
            "file": src_name,
            "from": from,
            "to": args.to,
            "ticks": "original",
            "cut_by": cmd,
            "created_unix": created,
        }),
    );

    let mut w = RecordingWriter::create_with_level(out, &header, SLICE_ZSTD_LEVEL)?;
    let (mut first, mut last, mut n) = (None::<u64>, 0u64, 0u64);
    let mut rebased = false;
    while let Some(r) = rec.next_raw() {
        let (t, line) = r?;
        if t > to {
            break;
        }
        if first.is_none() {
            first = Some(t);
            match timg.as_mut() {
                Some(img) => {
                    // The slice's first record carries the planes AS OF
                    // this tick: the accumulated image plus this record's
                    // own delta, written as a base.
                    let v: serde_json::Value =
                        serde_json::from_str(line).map_err(|e| format!("t={t}: {e}"))?;
                    let trec = TickRecord::from_value(&v)?;
                    if let Some(block) = &trec.terrain {
                        img.apply(block)
                            .map_err(|e| format!("t={t}: terrain: {e}"))?;
                    }
                    let blob = img.base_blob().ok_or_else(|| {
                        format!(
                            "terrain channel declared but no base record seen before t={t}; \
                             the slice's planes would be relative to an unknown origin"
                        )
                    })?;
                    let mut obj = v
                        .as_object()
                        .cloned()
                        .ok_or_else(|| format!("t={t}: record is not a JSON object"))?;
                    obj.insert(
                        "terrain".to_string(),
                        serde_json::json!({ "base_b64": b64_encode(&blob) }),
                    );
                    w.write_raw(&line_t_first(&obj)?)?;
                    rebased = true;
                }
                None => w.write_raw(line)?,
            }
        } else {
            w.write_raw(line)?;
        }
        last = t;
        n += 1;
    }
    w.finish()?;
    let Some(first) = first else {
        return Err(format!("{}: no record in t={from}..={to}", path.display()));
    };
    println!(
        "slice {} -> {}: {n} record(s), t={first}..={last} (asked {from}..{}), \
         terrain {}, ORIGINAL tick numbers kept",
        path.display(),
        out.display(),
        args.to
            .map(|t| t.to_string())
            .unwrap_or_else(|| "END".to_string()),
        if rebased {
            "measured, re-based at first record"
        } else {
            "none (no channel)"
        },
    );
    Ok(())
}

/// Serialise a record with `"t"` FIRST, whatever order the map holds:
/// the reader's prefix select (`{"t":N,`) — and `cut_mgcr.py` — read
/// the tick off the front of the line. Every piece goes through serde;
/// only the key order is ours.
fn line_t_first(obj: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let t = obj.get("t").ok_or("record without \"t\"")?;
    let mut s = String::with_capacity(obj.len() * 64);
    s.push_str("{\"t\":");
    s.push_str(&serde_json::to_string(t).map_err(|e| e.to_string())?);
    for (k, v) in obj {
        if k == "t" {
            continue;
        }
        s.push(',');
        s.push_str(&serde_json::to_string(k).map_err(|e| e.to_string())?);
        s.push(':');
        s.push_str(&serde_json::to_string(v).map_err(|e| e.to_string())?);
    }
    s.push('}');
    Ok(s)
}
