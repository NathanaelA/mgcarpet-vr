//! `MGC_ALLOC_TRACE` — the POOL ALLOCATOR microscope (round 98, dig Q13).
//!
//! Four independent census reads this campaign said the same thing: a
//! large share of what looks like arithmetic error is the port popping
//! pool slots in a different order from retail (dig 98-4's one-position
//! trail-ring rotation, round 97's `(9,9)` +4 meteor shift, mc2l22's
//! t=1200 missing/extra wall). Nobody had looked at the stacks
//! themselves — yet the recording CARRIES them
//! (`RetailMc2::free_stack` / `recycle_stack`, `mgcr.rs:2382`), so the
//! comparison is exact and free.
//!
//! `MGC_ALLOC_TRACE=<t0>:<t1>[:<depth>]` prints, at every graded
//! boundary in the window, retail's and the port's free and recycle
//! stacks TOP-FIRST (the next pop is element 0), plus the set
//! difference. Default-OFF, print-only: it changes nothing the grader
//! accepts.
use mgc_formats::mgcr::RetailMc2;
use mgc_sim::engine::world::World;

pub(crate) struct AllocTrace {
    t0: u64,
    t1: u64,
    depth: usize,
}

impl AllocTrace {
    pub(crate) fn from_env() -> Option<Self> {
        let v = std::env::var("MGC_ALLOC_TRACE").ok()?;
        let mut it = v.split(':');
        let t0 = it.next()?.parse().ok()?;
        let t1 = it.next().and_then(|s| s.parse().ok()).unwrap_or(t0);
        let depth = it.next().and_then(|s| s.parse().ok()).unwrap_or(16);
        Some(Self { t0, t1, depth })
    }

    pub(crate) fn emit(&self, world: &World, st: &RetailMc2, human: u16, t: u64) {
        if t < self.t0 || t > self.t1 {
            return;
        }
        let pool = st.ents.len();
        let keep = |s: &u16| (*s as usize) < pool && *s != human;
        let (pf, pr) = world.free_stacks_mc2();
        for (name, want, got) in [
            (
                "free",
                st.free_stack
                    .iter()
                    .copied()
                    .filter(keep)
                    .collect::<Vec<_>>(),
                pf.iter().copied().filter(keep).collect::<Vec<_>>(),
            ),
            (
                "recycle",
                st.recycle_stack
                    .iter()
                    .copied()
                    .filter(keep)
                    .collect::<Vec<_>>(),
                pr.iter().copied().filter(keep).collect::<Vec<_>>(),
            ),
        ] {
            let top = |v: &[u16]| -> String {
                v.iter()
                    .rev()
                    .take(self.depth)
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let ws: std::collections::BTreeSet<u16> = want.iter().copied().collect();
            let gs: std::collections::BTreeSet<u16> = got.iter().copied().collect();
            let only_r: Vec<u16> = ws.difference(&gs).copied().collect();
            let only_p: Vec<u16> = gs.difference(&ws).copied().collect();
            println!(
                "ALLOC t={t} {name}: retail len {} top [{}]\nALLOC t={t} {name}: port   len {} top [{}]",
                want.len(),
                top(&want),
                got.len(),
                top(&got),
            );
            if !only_r.is_empty() || !only_p.is_empty() {
                let cm = |s: u16| -> String {
                    let r = st
                        .ents
                        .get(s as usize)
                        .map_or("?".into(), |e| format!("({},{})", e.class3f, e.model40));
                    let p = world
                        .port_ent_lanes_mc2(s, human, false)
                        .map_or("?".into(), |l| {
                            let g = |n: &str| {
                                l.iter()
                                    .find(|(k, _)| *k == n)
                                    .and_then(|(_, v)| *v)
                                    .map_or("-".to_string(), |v| v.to_string())
                            };
                            format!("({},{})", g("class3f"), g("model40"))
                        });
                    format!("{s}:r{r}/p{p}")
                };
                println!(
                    "ALLOC t={t} {name}: ONLY-RETAIL {}",
                    only_r.iter().map(|&s| cm(s)).collect::<Vec<_>>().join(" ")
                );
                println!(
                    "ALLOC t={t} {name}: ONLY-PORT   {}",
                    only_p.iter().map(|&s| cm(s)).collect::<Vec<_>>().join(" ")
                );
            }
        }
    }
}

/// `MGC_STAGE_TRACE=<t0>[:<t1>]` — the OBJECTIVE BOARD microscope.
///
/// ⭐ `struct_0x3659C[local]` (`RetailMc2::objectives`, mgcr.rs:2419)
/// is RECORDED but NOTHING IN THE HARNESS GRADES IT. The pair importer
/// restores it (`conformance.rs:2658`), so pair mode is blind by
/// construction; the FREE RUN carries the port's own board for the
/// whole take. A class-11 model-32 switch fires its disposition the
/// tick `stage_0x3659F[par1]` reaches 2 (EF:54369), and a disposition
/// fire rebuilds BOTH allocator stacks and mass-spawns — so one early
/// board latch detonates the pool.
pub(crate) struct StageTrace {
    t0: u64,
    t1: u64,
    last: Option<(Vec<u8>, Vec<(u8, u8)>, usize)>,
}

impl StageTrace {
    pub(crate) fn from_env() -> Option<Self> {
        let v = std::env::var("MGC_STAGE_TRACE").ok()?;
        let mut it = v.split(':');
        let t0 = it.next()?.parse().ok()?;
        let t1 = it.next().and_then(|s| s.parse().ok()).unwrap_or(u64::MAX);
        Some(Self { t0, t1, last: None })
    }

    pub(crate) fn emit(&mut self, world: &World, st: &RetailMc2, t: u64) {
        if t < self.t0 || t > self.t1 {
            return;
        }
        let local = st.local_player as usize;
        let board = st.objectives.get(local).copied().unwrap_or([0u8; 11]);
        let (cur, rows) = world.mc2_objective_view();
        let key = (board.to_vec(), rows.clone(), cur);
        if self.last.as_ref() == Some(&key) && std::env::var("MGC_STAGE_TRACE_ALL").is_err() {
            return;
        }
        self.last = Some(key);
        println!(
            "STAGE t={t} retail board completed={} cur={} pause={} states={:?}",
            board[0],
            board[1],
            board[2],
            &board[3..]
        );
        println!(
            "STAGE t={t} port   board completed=? cur={cur} rows(kind,state)={:?}",
            rows
        );
        for (k, (kind, flags, bslot)) in st.stage_binds.iter().enumerate().take(8) {
            let Some(b) = bslot else {
                // ⚠⚠⚠ SAY SO. A row retail calls BOUND whose pointer
                // this decoder could not convert is the single most
                // important thing this trace can print: `mc2_pool_base`
                // recovers the pool base from the FREE stack's pointer
                // cells and falls back to the RECYCLE stack's, so a
                // frame on which BOTH are empty leaves every stage
                // bind unreadable — and `import_ent_mc2` then imports
                // "unreadable" as "unbound", killing the port's
                // type-1/2 objectives for exactly that pair. Silently
                // skipping the row printed "no news" where the news
                // was the whole story.
                if *flags & 1 != 0 {
                    println!(
                        "STAGE t={t}   row {k} kind={kind} bindflags={flags} \
                         bound=UNREADABLE (retail says BOUND; pool base unrecoverable — \
                         free stack {} / recycle {} cells)",
                        st.free_stack.len(),
                        st.recycle_stack.len(),
                    );
                }
                continue;
            };
            let r = st.ents.get(*b as usize);
            let pl = world.port_ent_lanes_mc2(*b, 0, false);
            let g = |n: &str| -> String {
                pl.as_ref()
                    .and_then(|l| l.iter().find(|(k, _)| *k == n).and_then(|(_, v)| *v))
                    .map_or("-".into(), |v| v.to_string())
            };
            println!(
                "STAGE t={t}   row {k} kind={kind} bindflags={flags} bound={b} \
                 retail life={} b3d={} class={} model={} b46={} reap={} | \
                 port life={} b3d={} class={} model={} f46={} reap={}",
                r.map_or(-1, |e| e.life as i64),
                r.map_or(-1, |e| e.b3d as i64),
                r.map_or(-1, |e| e.class3f as i64),
                r.map_or(-1, |e| e.model40 as i64),
                r.map_or(-1, |e| e.b46 as i64),
                r.map_or(-1, |e| ((e.flags >> 10) & 1) as i64),
                g("life"),
                g("b3d"),
                g("class3f"),
                g("model40"),
                g("b46"),
                g("flags.b1_reap4"),
            );
        }
    }
}
