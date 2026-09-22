//! PER-SLOT census: species + slot + field + tick ranges + owner.
//!
//!     slot_census_mc2 <take.mgcr> <replay --segmented --classify report>
//!
//! ⭐⭐⭐ THE COMPANION TO `head_census_mc2`, AND OFTEN THE ONE THAT
//! DECIDES THE DIG. The species census says "136 (5,25) heads"; this
//! one said "SIX Cymmerian slots inside one ~2,000-tick window", which
//! ruled out "one creature's whole life" immediately and pointed
//! straight at mc2l12's castle split (round 138, law 1). Run BOTH
//! before briefing anything.
use mgc_formats::mgcr::{Recording, decode_retail_mc2};
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let report = a.next().unwrap();
    let mut want: BTreeMap<u64, BTreeSet<(u16, String)>> = BTreeMap::new();
    let text = std::fs::read_to_string(&report).unwrap();
    let mut cur: Option<u64> = None;
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("pair ") {
            cur = rest
                .split('\u{2192}')
                .nth(1)
                .and_then(|t| t.trim_end_matches(':').trim().parse().ok());
            continue;
        }
        let Some(t) = cur else { continue };
        let l = line.trim();
        let slot = if let Some(r) = l.strip_prefix("slot ") {
            r.split_whitespace().next().and_then(|s| s.parse().ok())
        } else if let Some(i) = l.find("slot ") {
            l[i + 5..]
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        } else {
            None
        };
        let field = l
            .split_once(": retail")
            .and_then(|(lhs, _)| lhs.rsplit_once(' '))
            .map(|(_, f)| f.to_string())
            .unwrap_or_else(|| "<set>".into());
        if let Some(s) = slot {
            want.entry(t).or_default().insert((s, field));
        }
    }
    let mut rec = Recording::open(std::path::Path::new(&path)).unwrap();
    // (class,model,slot) -> (heads, fields, owner set, action set, ticks)
    let mut heads: BTreeMap<(u8, u8, u16), BTreeSet<u64>> = BTreeMap::new();
    let mut fields: BTreeMap<(u8, u8, u16), BTreeMap<String, usize>> = BTreeMap::new();
    let mut owners: BTreeMap<(u8, u8, u16), BTreeSet<u16>> = BTreeMap::new();
    let mut acts: BTreeMap<(u8, u8, u16), BTreeSet<u8>> = BTreeMap::new();
    while let Some(r) = rec.next_tick() {
        let tick = r.unwrap();
        let Some(slots) = want.get(&tick.t) else {
            continue;
        };
        let Some(raw) = tick.state.as_ref() else {
            continue;
        };
        let Ok(st) = decode_retail_mc2(raw) else {
            continue;
        };
        for (s, field) in slots {
            let Some(e) = st.ents.get(*s as usize) else {
                continue;
            };
            let k = (e.class3f as u8, e.model40 as u8, *s);
            heads.entry(k).or_default().insert(tick.t);
            *fields
                .entry(k)
                .or_default()
                .entry(field.clone())
                .or_default() += 1;
            owners.entry(k).or_default().insert(e.owner28);
            acts.entry(k).or_default().insert(e.action45);
        }
    }
    let mut out: Vec<_> = heads
        .iter()
        .map(|(k, h)| {
            (
                h.len(),
                *k,
                h.iter().copied().min().unwrap(),
                h.iter().copied().max().unwrap(),
            )
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    for (n, (c, m, s), t0, t1) in out {
        let k = (c, m, s);
        let mut fs: Vec<_> = fields[&k].iter().collect();
        fs.sort_by(|a, b| b.1.cmp(a.1));
        let top: Vec<String> = fs.iter().take(6).map(|(f, n)| format!("{f}×{n}")).collect();
        println!(
            "{n:>4} heads  ({c},{m}) slot {s:<4} t={t0}..{t1}  owner={:?} act={:?}  {}",
            owners[&k],
            acts[&k],
            top.join(" ")
        );
    }
}
