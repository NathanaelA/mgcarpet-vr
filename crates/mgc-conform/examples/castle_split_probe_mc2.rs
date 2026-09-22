//! CASTLE-SPLIT PROBE (MC2): read a take's retail states and report,
//! per tick, how many live `(3,2)` records each player owns next to
//! the player's own `CastleEntityIndex_0x3A_58` register — the
//! instrument for the MULTIPLE-CASTLES brain split (a player holding
//! two castles at once, or a register naming none while a castle
//! lives).
//!
//! Prints one line per CHANGE, not per tick: births, deaths and every
//! register move, so a 50k-tick take reads as a short ledger.
//!
//! usage: castle_split_probe_mc2 <mgcr>
use mgc_formats::mgcr::{Recording, decode_retail_mc2};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: castle_split_probe_mc2 <mgcr>");
    let mut rec = Recording::open(std::path::Path::new(&path)).expect("open");
    assert_eq!(rec.header.game, "mc2", "MC2 takes only");

    // (owner entity slot, register value, sorted castle slots) per player.
    let mut prev: Vec<(u16, i16, Vec<u16>)> = Vec::new();
    let mut first_split: Option<u64> = None;
    let mut first_orphan: Option<u64> = None;

    while let Some(r) = rec.next_tick() {
        let tick = r.expect("tick");
        let Some(raw) = tick.state.as_ref() else {
            continue;
        };
        let Ok(st) = decode_retail_mc2(raw) else {
            continue;
        };
        let t = tick.t;
        let mut now: Vec<(u16, i16, Vec<u16>)> = Vec::new();
        for p in &st.players {
            let own = p.play_index;
            // A castle's `id24` (@0x1A) names its owner CARPET slot.
            let mut castles: Vec<u16> = st
                .ents
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, e)| {
                    e.class3f == 3 && e.model40 == 2 && e.f1a == own && e.flags & 0x400 == 0
                })
                .map(|(j, _)| j as u16)
                .collect();
            castles.sort_unstable();
            now.push((own, p.castle_ent, castles));
        }
        if prev.len() != now.len() {
            prev = vec![(0, 0, Vec::new()); now.len()];
        }
        for (pi, cur) in now.iter().enumerate() {
            if *cur == prev[pi] {
                continue;
            }
            let (own, reg, ref castles) = *cur;
            let split = castles.len() > 1;
            let orphan = reg == 0 && !castles.is_empty();
            let tag = match (split, orphan) {
                (true, true) => " ⛔ SPLIT+ORPHAN",
                (true, false) => " ⛔ SPLIT",
                (false, true) => " ⚠ ORPHAN (register 0, castle alive)",
                _ => "",
            };
            if split && first_split.is_none() {
                first_split = Some(t);
            }
            if orphan && first_orphan.is_none() {
                first_orphan = Some(t);
            }
            println!("t={t} player {pi} (carpet {own}): register={reg} castles={castles:?}{tag}");
        }
        prev = now;
    }
    println!("--");
    match first_split {
        Some(t) => println!("FIRST TWO-CASTLE TICK: t={t}"),
        None => println!("no player ever held two live castles"),
    }
    match first_orphan {
        Some(t) => println!("FIRST ORPHANED-REGISTER TICK: t={t}"),
        None => println!("no player ever had a live castle with a zero register"),
    }
}
