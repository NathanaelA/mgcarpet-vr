//! HEAD CENSUS (MC2): take a `replay --segmented --classify` report
//! and say WHAT each divergent slot was — the (class, model) of every
//! slot named in a "first divergence" block, read out of the
//! recording's own retail state at that tick.
//!
//! The classify report names slots and fields but not species, so a
//! 700-head take reads as an undifferentiated wall. This turns it
//! into a census: which creature/token families own the heads, and
//! how many heads each family owns.
//!
//! usage: head_census_mc2 <mgcr> <classify-report.txt>
use mgc_formats::mgcr::{Recording, decode_retail_mc2};
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("usage: head_census_mc2 <mgcr> <report>");
    let report = a.next().expect("usage: head_census_mc2 <mgcr> <report>");

    // tick -> slots named in that tick's divergence block.
    let mut want: BTreeMap<u64, BTreeSet<(u16, String)>> = BTreeMap::new();
    let text = std::fs::read_to_string(&report).expect("report");
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
        // "slot 138 mana: retail …" / "missing in port: slot 18 (class …"
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
        // …and the FIELD that diverged, so the census can say
        // which lane of a species owns its heads.
        let field = l
            .split_once(": retail")
            .and_then(|(lhs, _)| lhs.rsplit_once(' '))
            .map(|(_, f)| f.to_string())
            .unwrap_or_else(|| "<set>".to_string());
        if let Some(s) = slot {
            want.entry(t).or_default().insert((s, field));
        }
    }
    eprintln!("{} divergent ticks named in {report}", want.len());

    let mut rec = Recording::open(std::path::Path::new(&path)).expect("open");
    // (class, model) -> (rows, distinct heads)
    let mut rows: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut heads: BTreeMap<(u8, u8), BTreeSet<u64>> = BTreeMap::new();
    let mut fields: BTreeMap<((u8, u8), String), usize> = BTreeMap::new();
    let mut seen = 0usize;
    while let Some(r) = rec.next_tick() {
        let tick = r.expect("tick");
        let Some(slots) = want.get(&tick.t) else {
            continue;
        };
        let Some(raw) = tick.state.as_ref() else {
            continue;
        };
        let Ok(st) = decode_retail_mc2(raw) else {
            continue;
        };
        seen += 1;
        for (s, field) in slots {
            let Some(e) = st.ents.get(*s as usize) else {
                continue;
            };
            let key = (e.class3f as u8, e.model40 as u8);
            *rows.entry(key).or_default() += 1;
            heads.entry(key).or_default().insert(tick.t);
            *fields.entry((key, field.clone())).or_default() += 1;
        }
    }
    eprintln!("{seen} of those ticks carried a decodable state\n");

    let mut out: Vec<_> = heads.iter().map(|(k, h)| (h.len(), rows[k], *k)).collect();
    out.sort_by(|a, b| b.cmp(a));
    println!("{:>6}  {:>6}  species", "heads", "rows");
    for (h, r, (c, m)) in out {
        let mut fs: Vec<_> = fields
            .iter()
            .filter(|((k, _), _)| *k == (c, m))
            .map(|((_, f), n)| (*n, f.as_str()))
            .collect();
        fs.sort_by(|a, b| b.cmp(a));
        let top: Vec<String> = fs.iter().take(5).map(|(n, f)| format!("{f}×{n}")).collect();
        println!("{h:>6}  {r:>6}  ({c},{m})  {}", top.join(" "));
    }
}
