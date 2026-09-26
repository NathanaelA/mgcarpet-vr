//! `init-check` — THE NATIVE-vs-RECORDED FIRST-STATE DIFFERENTIAL.
//!
//! Both graded runners SEED FROM THE RECORDING: `replay` imports the
//! take's first closure and free-runs, `verify-deltas` re-imports every
//! tick. So the harness grades the tick function and never the world
//! CONSTRUCTOR — a roster the level load never mints, a field no native
//! path ever writes, a seat the importer fills and `World::new` does
//! not are all invisible on every take, forever (mc2l15's lava moat and
//! the rival SPEED dispatch lane were both this class).
//!
//! `terrain-check` already closes that loop for the terrain planes:
//! build the level the way the port builds it, settle it by the
//! recorder's record-0 phase, diff against what retail had. This mode
//! is the same build ([`crate::native_settled_world`] — one settle, by
//! construction) diffed on everything ELSE record 0 holds: pool
//! occupancy, every per-entity lane, the wizard/brain block, the
//! objective board, the allocator stacks and the LCG.
//!
//! It grades nothing and gates nothing — it is a census. ⚠ The settle
//! flies an IDLE carpet at the authored start, so lanes the human's
//! first inputs touch (the carpet's own record, anything it aggroed)
//! are the player's, not the port's.

use crate::Args;
use mgc_formats::mgcr::{Family, Recording, decode_retail_mc1, decode_retail_mc2};
use mgc_sim::engine::world::conformance::retail_ent_lanes_mc2;
use std::collections::{BTreeMap, BTreeSet};

/// `(class, model)` per live retail slot (slot 0 is the scratch).
type Occupancy = BTreeMap<u16, (u8, u8)>;

struct Census {
    retail: Occupancy,
    port: Occupancy,
}

impl Census {
    /// (agree, differ, retail-only, port-only)
    fn slots(&self) -> (usize, usize, usize, usize) {
        let (mut same, mut differ, mut r_only) = (0, 0, 0);
        for (s, cm) in &self.retail {
            match self.port.get(s) {
                Some(p) if p == cm => same += 1,
                Some(_) => differ += 1,
                None => r_only += 1,
            }
        }
        let p_only = self
            .port
            .keys()
            .filter(|s| !self.retail.contains_key(s))
            .count();
        (same, differ, r_only, p_only)
    }

    /// Per-`(class, model)` head counts that disagree — slot-order
    /// independent, so it separates "minted a different NUMBER" from
    /// "minted the same things into different slots".
    fn population(&self) -> Vec<((u8, u8), usize, usize)> {
        let mut pop: BTreeMap<(u8, u8), (usize, usize)> = BTreeMap::new();
        for cm in self.retail.values() {
            pop.entry(*cm).or_default().0 += 1;
        }
        for cm in self.port.values() {
            pop.entry(*cm).or_default().1 += 1;
        }
        pop.into_iter()
            .filter(|(_, (r, p))| r != p)
            .map(|(k, (r, p))| (k, r, p))
            .collect()
    }
}

pub(crate) fn init_check(path: &std::path::Path, args: &Args) -> i32 {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    match run(path, args, &name) {
        Ok(identical) => {
            if identical {
                0
            } else {
                1
            }
        }
        Err(e) => {
            println!("INIT {name}: ERROR — {e}");
            2
        }
    }
}

fn run(path: &std::path::Path, args: &Args, name: &str) -> Result<bool, String> {
    let mut rec = Recording::open(path)?;
    let game = rec.header.game.clone();
    let family = rec.header.family()?;
    let level = rec.header.level.ok_or("recording has no level number")?;
    let first = rec
        .next_tick()
        .ok_or("empty recording")?
        .map_err(|e| e.to_string())?;
    let state = first
        .state
        .as_ref()
        .ok_or("record 0 carries no state channel")?;
    let (settle, corrected) = match args.settle {
        Some(n) => (n, false),
        None => crate::record0_settle(path, &first, family, &game, level, args)?,
    };
    let (world, _) = crate::native_settled_world(path, &first, family, &game, level, args, settle)?;
    let mut port: Occupancy = world
        .debug_pool_rows()
        .into_iter()
        .map(|(s, c, m, ..)| (s as u16, (c, m)))
        .collect();
    let mut shadow = crate::shadow::Shadow::census_all();
    let t = first.t;
    // (retail rand, human slot, retail occupancy, per-wizard retail seat)
    let (r_rand, human_slot, retail, seats): (u32, u16, Occupancy, Vec<(usize, u16)>) = match family
    {
        Family::Mc1 => {
            let st = decode_retail_mc1(state)?;
            let human = st
                .wizards
                .get(st.local_player as usize)
                .map_or(0, |w| w.play_index);
            shadow.compare_core_mc1(&world, &st, human, t);
            shadow.compare_ents_mc1(&world, &st, human, t);
            shadow.compare_wiz_mc1(&world, &st, t);
            shadow.compare_globals_mc1(&world, &st, t);
            shadow.compare_chains_mc1(&world, &st, human, t);
            shadow.compare_free_mc1(&world, &st, human, t);
            let occ = st
                .ents
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, e)| e.class64 != 0)
                .map(|(s, e)| (s as u16, (e.class64, e.model65)))
                .collect();
            let seats = st
                .wizards
                .iter()
                .enumerate()
                .filter(|(_, w)| w.play_index != 0)
                .map(|(i, w)| (i, w.play_index))
                .collect();
            (st.rand, human, occ, seats)
        }
        Family::Mc2 => {
            let st = decode_retail_mc2(state)?;
            let human = st
                .players
                .get(st.local_player as usize)
                .map_or(0, |p| p.play_index);
            let torn = BTreeSet::new();
            shadow.compare_ents_mc2(&world, &st, human, &torn, t);
            shadow.compare_map_heads_mc2(&world, &st, human, &torn, t);
            shadow.compare_wiz_mc2(&world, &st, t);
            shadow.compare_board_mc2(&world, &st, t);
            shadow.compare_free_mc2(&world, &st, human, t);
            // ⭐ `MGC_INIT_STACKS=<n>` — the allocator microscope for
            // the CONSTRUCTOR (`MGC_ALLOC_TRACE`'s twin: that one
            // only reaches graded boundaries, and a native build has
            // none). Prints the top n of both free stacks, next-pop
            // first, so a slot-order divergence with clean occupancy
            // names the transient that was born and freed in one
            // side only.
            if let Some(n) = std::env::var("MGC_INIT_STACKS")
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
            {
                let (pf, _) = world.free_stacks_mc2();
                let pool = st.ents.len();
                let cut = |v: &[u16]| {
                    v.iter()
                        .filter(|s| (**s as usize) < pool && **s != human)
                        .rev()
                        .take(n)
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                };
                println!("  FREE retail top{n}: {}", cut(&st.free_stack));
                println!("  FREE port   top{n}: {}", cut(pf));
            }
            let occ = st
                .ents
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, e)| e.class3f != 0)
                .map(|(s, e)| (s as u16, (e.class3f, e.model40)))
                .collect();
            let seats = st
                .players
                .iter()
                .enumerate()
                .filter(|(_, p)| p.play_index != 0)
                .map(|(i, p)| (i, p.play_index))
                .collect();
            (st.rand, human, occ, seats)
        }
    };
    let p_rand = world.rand_state();
    // The human's carpet is a pool record in retail and lives OUTSIDE
    // the pool in the port (`PLAYER_TARGET`) — a hole by design on
    // both sides of every comparison the harness makes, so it is not
    // an occupancy row. A PORT record sitting in that slot still is.
    let mut retail = retail;
    retail.remove(&human_slot);
    if port.get(&human_slot).is_some_and(|(c, _)| *c == 3) {
        port.remove(&human_slot);
    }
    let census = Census { retail, port };
    let (same, differ, r_only, p_only) = census.slots();
    let pop = census.population();
    // The port's seat per wizard, off the same projection the wizard
    // shadow walks — a rival seated in a different slot is SKIPPED by
    // that comparison (`play_index != ent`), so say so here or its
    // whole block reads clean where nobody looked.
    let port_seats: BTreeMap<u8, u16> = match family {
        Family::Mc1 => world
            .wiz_shadow_mc1()
            .iter()
            .map(|w| (w.wiz, w.ent))
            .collect(),
        Family::Mc2 => world
            .wiz_shadow_mc2()
            .iter()
            .map(|w| (w.wiz, w.ent))
            .collect(),
    };

    println!(
        "== init-check {} (game {game}, level {level}, record 0 @t={t}, port settled {settle} \
         tick(s){})",
        path.display(),
        if corrected {
            " = recorder phase CORRECTED BY THE LCG (the +63 phase read one off)"
        } else if args.settle.is_none() {
            " = recorder phase"
        } else {
            ""
        }
    );
    println!(
        "  rand: retail {r_rand:#010x} port {p_rand:#010x} — {}",
        if r_rand == p_rand {
            "MATCH"
        } else {
            "DIFFERENT"
        }
    );
    println!(
        "  pool: retail {} live, port {} live (human slot {human_slot}) — {same} slot(s) agree on \
         (class,model), {differ} differ, {r_only} retail-only, {p_only} port-only",
        census.retail.len(),
        census.port.len(),
    );
    if !pop.is_empty() {
        println!("  population (class,model) retail/port, where different:");
        for ((c, m), r, p) in &pop {
            println!("    ({c:>3},{m:>3}): retail {r} port {p}");
        }
    }
    let mut shown = 0;
    let all: BTreeSet<u16> = census
        .retail
        .keys()
        .chain(census.port.keys())
        .copied()
        .collect();
    for s in all {
        let (r, p) = (census.retail.get(&s), census.port.get(&s));
        if r == p {
            continue;
        }
        if shown == 0 {
            println!("  slot disagreements (first {}):", args.max_diffs);
        }
        if shown >= args.max_diffs {
            break;
        }
        shown += 1;
        let f = |v: Option<&(u8, u8)>| v.map_or("—".to_string(), |(c, m)| format!("({c},{m})"));
        println!("    slot {s}: retail {} port {}", f(r), f(p));
    }
    let mut seat_diff = 0;
    for (wiz, slot) in &seats {
        let ps = port_seats.get(&(*wiz as u8)).copied();
        // wiz 0's port seat is the PLAYER_TARGET sentinel by design.
        let ok = *wiz == 0 || ps == Some(*slot);
        if !ok {
            seat_diff += 1;
        }
        println!(
            "  seat wiz {wiz}: retail slot {slot} port {}{}",
            ps.map_or("— (no live wizard)".to_string(), |s| s.to_string()),
            if ok {
                ""
            } else {
                "  ⚠ brain block NOT compared"
            }
        );
    }
    for wiz in port_seats.keys() {
        if !seats.iter().any(|(w, _)| *w as u8 == *wiz) {
            seat_diff += 1;
            println!(
                "  seat wiz {wiz}: retail — port {}  ⚠ port-only wizard",
                port_seats[wiz]
            );
        }
    }
    print!("{}", shadow.render(false));

    // `MGC_INIT_DUMP=<slot>[,<slot>…]` — the native world's lanes beside
    // retail's for a named slot list, ≠-marked. `dump-state --port`
    // free-runs from an IMPORT, so it cannot show a constructor row.
    if let Ok(list) = std::env::var("MGC_INIT_DUMP")
        && family == Family::Mc2
    {
        let st = decode_retail_mc2(state)?;
        for (i, row) in st.stagevars.iter().enumerate() {
            if row[0] & 0xF == 0 {
                continue;
            }
            println!(
                "  -- stagevar[{i}] kind {} flags {:#04x} chain {} cadence {} payload {}                  (watch_ent decoded {})",
                row[0] & 0xF,
                row[1],
                row[2],
                row[3],
                u32::from_le_bytes([row[4], row[5], row[6], row[7]]),
                st.stagevar_watch[i],
            );
        }
        for s in list.split(',').filter_map(|s| s.trim().parse::<u16>().ok()) {
            let Some(re) = st.ents.get(s as usize) else {
                continue;
            };
            let port: BTreeMap<&'static str, Option<i64>> = world
                .port_ent_lanes_mc2(s, human_slot, false)
                .unwrap_or_default()
                .into_iter()
                .collect();
            println!("  -- slot {s} (class {}, model {})", re.class3f, re.model40);
            for (name, rv) in retail_ent_lanes_mc2(re) {
                let pv = port.get(name).copied().flatten();
                let mark = if pv == Some(rv) { " " } else { "≠" };
                println!(
                    "     {mark} {name:>10}: retail {rv:>8}  port {}",
                    pv.map_or("—".to_string(), |v| v.to_string())
                );
            }
        }
    }

    let lane_rows: u64 = shadow.lanes.values().map(|l| l.rows).sum();
    let wiz_rows: u64 = shadow.wiz_lanes.values().map(|l| l.rows).sum();
    let board_rows: u64 = shadow.board.values().map(|l| l.rows).sum();
    let identical = r_rand == p_rand
        && differ + r_only + p_only == 0
        && seat_diff == 0
        && lane_rows + wiz_rows + board_rows == 0
        && shadow.free.0 + shadow.recycle.0 == 0;
    println!(
        "INIT {name}: {} — settle {settle} · rand {} · slots {same} agree / {differ} differ / \
         {r_only} retail-only / {p_only} port-only · {} population row(s) · ent lanes {lane_rows} \
         row(s) in {} lane(s) · wiz {wiz_rows} · seats off {seat_diff} · board {board_rows} · \
         free {} · recycle {}",
        if identical { "IDENTICAL" } else { "DIFFERENT" },
        if r_rand == p_rand { "MATCH" } else { "DIFF" },
        pop.len(),
        shadow.lanes.len(),
        if shadow.free.0 == 0 { "MATCH" } else { "DIFF" },
        if shadow.recycle.0 == 0 {
            "MATCH"
        } else {
            "DIFF"
        },
    );
    Ok(identical)
}
