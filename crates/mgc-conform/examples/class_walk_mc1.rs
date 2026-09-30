//! Per-tick CHANGE LOG of every record that matches a filter — the
//! retail side only. Each matching slot is printed in full the first
//! tick it matches and then field by field as its lanes move.
//! Usage: class_walk_mc1 <mgcr> [--from t] [--to t] [--class c]
//!        [--model m] [--owner id24] [--slot s]… [--quiet f63,next20,…]
//!        [--wiz n]…   (a wizard block, keyed 100000 + n)
use mgc_formats::mgcr::{Recording, decode_retail_mc1};
use std::collections::HashMap;

fn rows_of(text: &str) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut key = String::new();
    for line in text.lines() {
        let l = line.trim().trim_end_matches(',');
        if let Some((k, v)) = l.split_once(": ") {
            if !v.ends_with('[') && !v.ends_with('(') && !v.ends_with('{') {
                rows.push((k.to_string(), v.to_string()));
                continue;
            }
            key = k.to_string();
            rows.push((key.clone(), String::new()));
        } else if !l.is_empty() && l != "}" && !l.ends_with('{') {
            if let Some(r) = rows.last_mut() {
                if r.0 == key {
                    r.1.push_str(l);
                    r.1.push(' ');
                }
            }
        }
    }
    rows
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect(
        "usage: class_walk_mc1 <mgcr> [--from t] [--to t] [--class c] [--model m] [--owner id] [--slot s]…",
    );
    let (mut from, mut to) = (0u64, u64::MAX);
    let (mut class, mut model, mut owner): (Option<u8>, Option<u8>, Option<u16>) =
        (None, None, None);
    let mut slots: Vec<usize> = Vec::new();
    let mut wizs: Vec<usize> = Vec::new();
    let mut quiet: Vec<String> = ["f63", "next20", "prev22", "chain_next"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rest: Vec<String> = args.collect();
    let mut i = 0;
    while i + 1 < rest.len() {
        let v = &rest[i + 1];
        match rest[i].as_str() {
            "--from" => from = v.parse().unwrap(),
            "--to" => to = v.parse().unwrap(),
            "--class" => class = Some(v.parse().unwrap()),
            "--model" => model = Some(v.parse().unwrap()),
            "--owner" => owner = Some(v.parse().unwrap()),
            "--slot" => slots.push(v.parse().unwrap()),
            "--wiz" => wizs.push(v.parse().unwrap()),
            "--quiet" => quiet = v.split(',').map(|s| s.to_string()).collect(),
            other => panic!("unknown switch {other}"),
        }
        i += 2;
    }
    let mut last: HashMap<usize, Vec<(String, String)>> = HashMap::new();
    let mut rec = Recording::open(std::path::Path::new(&path)).expect("open");
    while let Some(r) = rec.next_tick() {
        let tick = r.expect("tick");
        if tick.t < from {
            continue;
        }
        if tick.t > to {
            break;
        }
        let Some(state) = &tick.state else { continue };
        let st = decode_retail_mc1(state).expect("decode");
        for &n in &wizs {
            let Some(w) = st.wizards.get(n) else { continue };
            let rows = rows_of(&format!("{w:#?}"));
            if let Some(prev) = last.get(&(100_000 + n)) {
                let ch: Vec<String> = rows
                    .iter()
                    .zip(prev.iter())
                    .filter(|(a, b)| a.1 != b.1 && !quiet.contains(&a.0))
                    .map(|(a, b)| format!("{} {} -> {}", a.0, b.1, a.1))
                    .collect();
                if !ch.is_empty() {
                    println!("t={} wiz {n}: {}", tick.t, ch.join(", "));
                }
            } else {
                let all: Vec<String> = rows
                    .iter()
                    .filter(|(_, v)| v != "0" && !v.is_empty())
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect();
                println!("t={} wiz {n} ENTERS: {}", tick.t, all.join(", "));
            }
            last.insert(100_000 + n, rows);
        }
        for (s, e) in st.ents.iter().enumerate() {
            let hit = if !slots.is_empty() {
                slots.contains(&s)
            } else {
                class.is_none_or(|c| e.class64 == c)
                    && model.is_none_or(|m| e.model65 == m)
                    && owner.is_none_or(|o| e.id24 == o)
                    && (class.is_some() || e.class64 != 0)
            };
            if !hit {
                if last.remove(&s).is_some() {
                    println!("t={} slot {s}: LEFT the filter ({},{})", tick.t, e.class64, e.model65);
                }
                continue;
            }
            let rows = rows_of(&format!("{e:#?}"));
            match last.get(&s) {
                None => {
                    let all: Vec<String> = rows
                        .iter()
                        .filter(|(_, v)| v != "0" && !v.is_empty())
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect();
                    println!("t={} slot {s} ENTERS: {}", tick.t, all.join(", "));
                }
                Some(prev) => {
                    let ch: Vec<String> = rows
                        .iter()
                        .zip(prev.iter())
                        .filter(|(a, b)| a.1 != b.1 && !quiet.contains(&a.0))
                        .map(|(a, b)| format!("{} {} -> {}", a.0, b.1, a.1))
                        .collect();
                    if !ch.is_empty() {
                        println!("t={} slot {s} ({},{}): {}", tick.t, e.class64, e.model65, ch.join(", "));
                    }
                }
            }
            last.insert(s, rows);
        }
    }
}
