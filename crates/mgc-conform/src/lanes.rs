//! `lane-check` — THE CENSUS OF THE LANES THE 2026-09-27 RECORDER ADDED
//! and no consumer read: the INIT RECORD, the ENTITY-INDEX planes and
//! `terrain_rand` (docs/RECORDING.md "The init record", "The entity
//! index").
//!
//! It grades nothing and gates nothing — a census, the `init-check`
//! contract. Four blocks, one verdict line each (`LANE <take> <lane>:`):
//!
//! - **INDEX** (retail against itself, every tick): the captured
//!   `mapEntityIndex` table walked through the pool's own links. The
//!   port has always RECONSTRUCTED the heads from the pool; this says
//!   which of that reconstruction's assumptions hold.
//! - **INDEX-IMPORT** (the importer against the capture, every
//!   `--sample-every`-th tick): import state@t the way every anchor
//!   does and read the port's head table beside retail's.
//! - **TRAND** (retail against itself, every tick): the retile LCG's
//!   per-tick draw count, beside the terrain channel's own edits.
//! - **INIT** (retail's init record beside record 0, and the port's
//!   native build beside both): what frame 1 did, and what the port's
//!   constructor holds in the new lanes.

use crate::Args;
use mgc_formats::mgcr::{
    Family, Recording, RetailMc1, RetailMc2, TerrainImage, decode_retail_mc1, decode_retail_mc2,
    decode_terrain_delta,
};
use std::collections::BTreeMap;

/// One pool record, reduced to what the tile chains read.
#[derive(Clone, Copy, Default)]
pub(crate) struct Link {
    class: u8,
    model: u8,
    linked: bool,
    /// MC2's disable bit (`flags & 0x400`): the record waits for the
    /// next frame's top reap. MC1 has no such class.
    ghost: bool,
    next: u16,
    prev: u16,
    cell: u16,
}

/// The pool of one record, family-blind.
pub(crate) struct Pool {
    ents: Vec<Link>,
    human: u16,
    rand: u32,
}

pub(crate) fn pool_mc1(st: &RetailMc1) -> Pool {
    Pool {
        ents: st
            .ents
            .iter()
            .map(|e| Link {
                class: e.class64,
                model: e.model65,
                linked: e.flags & 4 != 0,
                ghost: false,
                next: e.next20,
                prev: e.prev22,
                cell: (e.y >> 8) << 8 | (e.x >> 8),
            })
            .collect(),
        human: st
            .wizards
            .get(st.local_player as usize)
            .map_or(0, |w| w.play_index),
        rand: st.rand,
    }
}

pub(crate) fn pool_mc2(st: &RetailMc2) -> Pool {
    Pool {
        ents: st
            .ents
            .iter()
            .map(|e| Link {
                class: e.class3f,
                model: e.model40,
                linked: e.flags & 4 != 0,
                ghost: e.flags & 0x400 != 0,
                next: e.next16,
                prev: e.prev18,
                cell: (e.y >> 8) << 8 | (e.x >> 8),
            })
            .collect(),
        human: st
            .players
            .get(st.local_player as usize)
            .map_or(0, |p| p.play_index),
        rand: st.rand,
    }
}

fn pool_of(family: Family, state: &[u8]) -> Result<Pool, String> {
    Ok(match family {
        Family::Mc1 => pool_mc1(&decode_retail_mc1(state)?),
        Family::Mc2 => pool_mc2(&decode_retail_mc2(state)?),
    })
}

/// A tally with its first witness.
#[derive(Default, Clone)]
struct Tally {
    rows: u64,
    ticks: u64,
    first: Option<String>,
    /// `(class, model)` of the record the row is about.
    by_cm: BTreeMap<(u8, u8), u64>,
    last_t: Option<u64>,
}

impl Tally {
    /// [`Self::hit`], and the row itself on stdout under
    /// `MGC_INDEX_VERBOSE` (the first 400 of a run).
    fn hit_loud(&mut self, t: u64, cm: (u8, u8), say: impl Fn() -> String) {
        static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if *V.get_or_init(|| std::env::var_os("MGC_INDEX_VERBOSE").is_some())
            && N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 400
        {
            println!("INDEXROW {}", say());
        }
        self.hit(t, cm, say);
    }

    fn hit(&mut self, t: u64, cm: (u8, u8), say: impl FnOnce() -> String) {
        self.rows += 1;
        if self.last_t != Some(t) {
            self.ticks += 1;
            self.last_t = Some(t);
        }
        *self.by_cm.entry(cm).or_default() += 1;
        if self.first.is_none() {
            self.first = Some(say());
        }
    }

    fn render(&self, name: &str) -> Option<String> {
        if self.rows == 0 {
            return None;
        }
        let mut cm: Vec<_> = self.by_cm.iter().collect();
        cm.sort_by(|a, b| b.1.cmp(a.1));
        let cm: Vec<String> = cm
            .iter()
            .take(6)
            .map(|((c, m), n)| format!("({c},{m})×{n}"))
            .collect();
        Some(format!(
            "    {name}: {} row(s) on {} tick(s) — {}  e.g. {}",
            self.rows,
            self.ticks,
            cm.join(" "),
            self.first.as_deref().unwrap_or("")
        ))
    }
}

/// The captured head table of one terrain image.
fn index_of(img: &TerrainImage) -> Option<Vec<u16>> {
    let lo = img.plane("entity_index_lo")?;
    let hi = img.plane("entity_index_hi")?;
    Some(
        lo.iter()
            .zip(hi)
            .map(|(&l, &h)| l as u16 | (h as u16) << 8)
            .collect(),
    )
}

/// The index planes of a full plane image (the init record's
/// `terrain_b64`), by the header's plane order.
fn index_of_blob(planes: &[String], cells: usize, blob: &[u8]) -> Option<Vec<u16>> {
    let at = |name: &str| {
        let i = planes.iter().position(|p| p == name)?;
        blob.get(i * cells..(i + 1) * cells)
    };
    let (lo, hi) = (at("entity_index_lo")?, at("entity_index_hi")?);
    Some(
        lo.iter()
            .zip(hi)
            .map(|(&l, &h)| l as u16 | (h as u16) << 8)
            .collect(),
    )
}

/// THE INTEGRITY WALK. Every category is a sentence the port's
/// reconstruction assumes and never measured.
#[derive(Default)]
struct IndexCensus {
    ticks: u64,
    heads: u64,
    members: u64,
    /// The head slot is past the pool.
    head_oob: Tally,
    /// A chain member (head included) is a FREE record (class 0).
    dead_member: Tally,
    /// A chain member is live and its link bit is clear.
    unlinked_member: Tally,
    /// A member's position names another cell than the chain's.
    wrong_cell: Tally,
    /// A member's back link does not name the member before it.
    prev_break: Tally,
    /// The walk came back to a record it had passed.
    cycle: Tally,
    /// A live, linked record no chain reaches.
    orphan: Tally,
    /// A record two chains reach.
    shared: Tally,
    /// THE SHADOW'S RULE — "every live linked record with no back link
    /// heads the chain of its own cell" — against the capture.
    rule_cap_only: Tally,
    rule_der_only: Tally,
    rule_differ: Tally,
}

impl IndexCensus {
    fn walk(&mut self, t: u64, idx: &[u16], pool: &Pool) {
        self.ticks += 1;
        let n = pool.ents.len();
        let mut seen = vec![0u16; n];
        let cm = |s: usize| (pool.ents[s].class, pool.ents[s].model);
        for (cell, &h) in idx.iter().enumerate() {
            if h == 0 {
                continue;
            }
            self.heads += 1;
            if h as usize >= n {
                self.head_oob
                    .hit(t, (0, 0), || format!("t={t} cell {cell} head {h}"));
                continue;
            }
            let (mut cur, mut before, mut steps) = (h as usize, 0usize, 0usize);
            while cur != 0 && cur < n {
                let e = pool.ents[cur];
                if seen[cur] != 0 {
                    if seen[cur] as usize == cell + 1 || steps > 0 && seen[cur] as usize == cell + 1
                    {
                        self.cycle.hit(t, cm(cur), || {
                            format!("t={t} cell {cell} head {h} re-enters slot {cur}")
                        });
                    } else {
                        self.shared.hit(t, cm(cur), || {
                            format!(
                                "t={t} slot {cur} in the chains of cells {} and {cell}",
                                seen[cur] - 1
                            )
                        });
                    }
                    break;
                }
                // cell + 1 so cell 0 is distinguishable from "unseen";
                // cell 65535 saturates and is reported as shared at
                // worst (no record stands on it in the corpus).
                seen[cur] = (cell as u16).saturating_add(1);
                self.members += 1;
                if e.class == 0 {
                    self.dead_member.hit(t, cm(cur), || {
                        format!(
                            "t={t} cell {cell} head {h}: slot {cur} is FREE (depth {steps}, next \
                             {}, prev {})",
                            e.next, e.prev
                        )
                    });
                } else if !e.linked {
                    self.unlinked_member.hit(t, cm(cur), || {
                        format!(
                            "t={t} cell {cell} head {h}: slot {cur} ({},{}) link bit clear (depth \
                             {steps})",
                            e.class, e.model
                        )
                    });
                }
                if e.class != 0 && e.cell as usize != cell {
                    self.wrong_cell.hit(t, cm(cur), || {
                        format!(
                            "t={t} slot {cur} ({},{}) stands on cell {} and hangs under {cell}",
                            e.class, e.model, e.cell
                        )
                    });
                }
                if e.prev as usize != before {
                    self.prev_break.hit(t, cm(cur), || {
                        format!(
                            "t={t} cell {cell}: slot {cur} ({},{}) prev {} but the walk came from \
                             {before}",
                            e.class, e.model, e.prev
                        )
                    });
                }
                before = cur;
                cur = e.next as usize;
                steps += 1;
                if steps > n {
                    break;
                }
            }
        }
        for s in 1..n {
            let e = pool.ents[s];
            if e.class != 0 && e.linked && seen[s] == 0 {
                self.orphan.hit(t, cm(s), || {
                    format!(
                        "t={t} slot {s} ({},{}) on cell {} (index there: {}), next {} prev {}",
                        e.class, e.model, e.cell, idx[e.cell as usize], e.next, e.prev
                    )
                });
            }
        }
        // The shadow's rule.
        let mut derived = vec![0u16; idx.len()];
        for s in 1..n {
            let e = pool.ents[s];
            if e.class != 0 && e.linked && e.prev == 0 {
                derived[e.cell as usize] = s as u16;
            }
        }
        for (cell, (&c, &d)) in idx.iter().zip(&derived).enumerate() {
            if c == d {
                continue;
            }
            let who = if c != 0 && (c as usize) < n { c } else { d } as usize;
            let say = || format!("t={t} cell {cell}: captured {c} derived {d}");
            if d == 0 {
                self.rule_cap_only.hit(t, cm(who), say);
            } else if c == 0 {
                self.rule_der_only.hit(t, cm(who), say);
            } else {
                self.rule_differ.hit(t, cm(who), say);
            }
        }
    }

    fn rows(&self) -> u64 {
        [
            &self.head_oob,
            &self.dead_member,
            &self.unlinked_member,
            &self.wrong_cell,
            &self.prev_break,
            &self.cycle,
            &self.orphan,
            &self.shared,
        ]
        .iter()
        .map(|t| t.rows)
        .sum()
    }

    fn rule_rows(&self) -> u64 {
        self.rule_cap_only.rows + self.rule_der_only.rows + self.rule_differ.rows
    }
}

/// `pseudo = pseudo * 9377 + 9439` (u16) is a full-period LCG, so the
/// draw count between two states is unique: the difference of their
/// ranks on the cycle.
pub(crate) fn lcg_rank() -> Vec<u16> {
    let mut rank = vec![0u16; 65536];
    let mut x = 0u16;
    for k in 0..65536u32 {
        rank[x as usize] = k as u16;
        x = x.wrapping_mul(9377).wrapping_add(9439);
    }
    rank
}

#[derive(Default)]
struct RandCensus {
    pairs: u64,
    draw_ticks: u64,
    draws: u64,
    max: (u16, u64),
    /// Draws on a tick whose terrain delta is empty on every terrain
    /// plane (the index planes are not terrain).
    silent: Tally,
    /// A `type`/`angle` edit on a tick the LCG did not move.
    undrawn: Tally,
    /// Draw-count histogram.
    hist: BTreeMap<u16, u64>,
}

pub(crate) fn lane_check(path: &std::path::Path, args: &Args) -> i32 {
    let name = crate::verify::take_stem(path);
    match run(path, args, &name) {
        Ok(()) => 0,
        Err(e) => {
            println!("LANE {name}: ERROR — {e}");
            2
        }
    }
}

fn run(path: &std::path::Path, args: &Args, name: &str) -> Result<(), String> {
    let mut rec = Recording::open(path)?;
    crate::take_binary(&rec);
    let family = rec.header.family()?;
    let game = rec.header.game.clone();
    let level = rec.header.level.ok_or("recording has no level number")?;
    let decl = rec
        .header
        .channels
        .terrain
        .clone()
        .ok_or("no terrain channel")?;
    let cells = decl.cells();
    let has_index = decl.planes.iter().any(|p| p == "entity_index_lo");
    let init = rec.init.clone();
    if !has_index && init.is_none() {
        println!("LANE {name}: NONE — the take carries no init record and no index planes");
        return Ok(());
    }
    let terrain_planes: Vec<usize> = decl
        .planes
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.starts_with("entity_index"))
        .map(|(i, _)| i)
        .collect();
    let paint_planes: Vec<usize> = decl
        .planes
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.as_str(), "type" | "angle"))
        .map(|(i, _)| i)
        .collect();

    let mut timg = TerrainImage::new(&decl);
    let mut index = IndexCensus::default();
    let mut rand = RandCensus::default();
    let rank = lcg_rank();
    let mut prev: Option<(u64, u16)> = None;
    let mut first: Option<mgc_formats::mgcr::TickRecord> = None;
    let mut first_planes: Option<Vec<Vec<u8>>> = None;
    let mut first_index: Option<Vec<u16>> = None;
    let mut n = 0u64;

    // The importer's side: one world, re-anchored at every sample.
    let stride = args.sample_every.max(1);
    let mut import = ImportCheck::new(path, family, &game, level, args)?;

    while let Some(r) = rec.next_tick() {
        let tick = r?;
        if args.limit.is_some_and(|l| n >= l) {
            break;
        }
        n += 1;
        let t = tick.t;
        // (terrain edits, paint edits) this record's delta carries.
        let mut edits = (0usize, 0usize);
        if let Some(block) = &tick.terrain {
            if let Some(delta) = &block.delta {
                let d = decode_terrain_delta(delta, decl.planes.len(), cells)?;
                edits.0 = terrain_planes.iter().map(|&i| d[i].len()).sum();
                edits.1 = paint_planes.iter().map(|&i| d[i].len()).sum();
            }
            timg.apply(block)
                .map_err(|e| format!("t={t}: terrain: {e}"))?;
        }
        let Some(state) = &tick.state else {
            prev = None;
            continue;
        };
        let pool = pool_of(family, state)?;
        if first.is_none() {
            first_planes = Some(
                decl.planes
                    .iter()
                    .map(|p| timg.plane(p).map(<[u8]>::to_vec).unwrap_or_default())
                    .collect(),
            );
            first_index = index_of(&timg);
            first = Some(tick.clone());
        }
        if has_index && timg.based() {
            let idx = index_of(&timg).ok_or("index planes declared and not in the image")?;
            index.walk(t, &idx, &pool);
            if (n - 1) % stride == 0 {
                import.sample(t, &idx, &pool, state, &timg)?;
            }
        }
        if let Some(r1) = tick.terrain_rand {
            if let Some((pt, r0)) = prev
                && pt + 1 == t
            {
                rand.pairs += 1;
                let d = rank[r1 as usize].wrapping_sub(rank[r0 as usize]);
                if d != 0 {
                    rand.draw_ticks += 1;
                    rand.draws += d as u64;
                    *rand.hist.entry(d).or_default() += 1;
                    if d > rand.max.0 {
                        rand.max = (d, t);
                    }
                    if edits.0 == 0 {
                        rand.silent.hit(t, (0, 0), || {
                            format!("t={t}: {d} draw(s), terrain delta empty")
                        });
                    }
                } else if edits.1 != 0 {
                    rand.undrawn.hit(t, (0, 0), || {
                        format!("t={t}: {} type/angle edit(s), LCG still", edits.1)
                    });
                }
            }
            prev = Some((t, r1));
        } else {
            prev = None;
        }
    }
    let first = first.ok_or("the take holds no record with a state")?;

    // ---- INDEX ---------------------------------------------------------
    if has_index {
        println!(
            "== lane-check {} (game {game}, level {level}) — {} tick(s), {} head(s), {} chain \
             member(s) walked",
            path.display(),
            index.ticks,
            index.heads,
            index.members
        );
        for (nm, tl) in [
            ("head past the pool", &index.head_oob),
            ("FREE record in a chain", &index.dead_member),
            ("link bit clear in a chain", &index.unlinked_member),
            ("member stands on another cell", &index.wrong_cell),
            ("back link broken", &index.prev_break),
            ("cycle", &index.cycle),
            ("record in two chains", &index.shared),
            ("linked live record no chain reaches", &index.orphan),
            ("RULE captured head, none derived", &index.rule_cap_only),
            ("RULE derived head, none captured", &index.rule_der_only),
            ("RULE heads differ", &index.rule_differ),
        ] {
            if let Some(l) = tl.render(nm) {
                println!("{l}");
            }
        }
        println!(
            "LANE {name} INDEX: {} — {} tick(s) · integrity {} row(s) · prev-0 rule {} row(s)",
            if index.rows() + index.rule_rows() == 0 {
                "CONSISTENT"
            } else {
                "ROWS"
            },
            index.ticks,
            index.rows(),
            index.rule_rows()
        );
        for l in import.render() {
            println!("{l}");
        }
        println!(
            "LANE {name} INDEX-IMPORT: {} — {} sample(s) (every {stride}) · head {} row(s) on {} \
             sample(s) · order {} row(s) · disabled-record {} row(s) · human seat {} row(s)",
            if import.heads.rows + import.order.rows + import.seat.rows == 0 {
                "IDENTICAL"
            } else {
                "ROWS"
            },
            import.samples,
            import.heads.rows,
            import.heads.ticks,
            import.order.rows,
            import.ghost.rows,
            import.seat.rows
        );
    }

    // ---- TRAND ---------------------------------------------------------
    if prev.is_some() || rand.pairs != 0 {
        let hist: Vec<String> = rand
            .hist
            .iter()
            .take(12)
            .map(|(d, c)| format!("{d}×{c}"))
            .collect();
        if !hist.is_empty() {
            println!("    draws per tick (draws×ticks): {}", hist.join(" "));
        }
        for (nm, tl) in [
            ("draws with an empty terrain delta", &rand.silent),
            ("type/angle edit with the LCG still", &rand.undrawn),
        ] {
            if let Some(l) = tl.render(nm) {
                println!("{l}");
            }
        }
        println!(
            "LANE {name} TRAND: {} pair(s) · {} tick(s) drew · {} draw(s) · max {} @t={} · silent \
             {} · undrawn {}",
            rand.pairs,
            rand.draw_ticks,
            rand.draws,
            rand.max.0,
            rand.max.1,
            rand.silent.ticks,
            rand.undrawn.ticks
        );
    } else {
        println!("LANE {name} TRAND: NONE — no terrain_rand in the take");
    }

    // ---- INIT ----------------------------------------------------------
    init_block(
        path,
        args,
        name,
        family,
        &game,
        level,
        &decl.planes,
        cells,
        init.as_ref(),
        &first,
        first_planes.as_deref(),
        first_index.as_deref(),
        &rank,
    )
}

/// The importer against the captured head table.
struct ImportCheck {
    family: Family,
    world: mgc_sim::engine::world::World,
    pristine: mgc_sim::engine::features::Planes,
    things: Option<mgc_sim::engine::world::conformance::ThingTable>,
    samples: u64,
    heads: Tally,
    /// The walk order under a head both sides agree on.
    order: Tally,
    /// Rows the importer's GHOST rule explains: retail's chain with
    /// every disabled record taken out is the port's chain.
    ghost: Tally,
    /// The human's imported seat against his own record's cell and
    /// `next` link (a successor retail's table holds soundly).
    seat: Tally,
}

impl ImportCheck {
    fn new(
        path: &std::path::Path,
        family: Family,
        game: &str,
        level: u32,
        args: &Args,
    ) -> Result<Self, String> {
        let (world, pristine, things) = match family {
            Family::Mc1 => {
                let (w, p) = crate::verify::build_world(&args.baked, game, level)?;
                (w, p, None)
            }
            Family::Mc2 => {
                let replayed = crate::verify_mc2::mc2_take_replayed(path)?;
                let (w, p, t) = crate::verify_mc2::build_world_mc2(&args.baked, level, replayed)?;
                (w, p, Some(t))
            }
        };
        Ok(Self {
            family,
            world,
            pristine,
            things,
            samples: 0,
            heads: Tally::default(),
            order: Tally::default(),
            ghost: Tally::default(),
            seat: Tally::default(),
        })
    }

    fn sample(
        &mut self,
        t: u64,
        idx: &[u16],
        pool: &Pool,
        state: &[u8],
        timg: &TerrainImage,
    ) -> Result<(), String> {
        self.samples += 1;
        let img = Some(timg.clone());
        match self.family {
            Family::Mc1 => {
                let st = decode_retail_mc1(state)?;
                crate::replay::anchor_mc1(&mut self.world, &self.pristine, &img, &st, t)?;
            }
            Family::Mc2 => {
                let st = decode_retail_mc2(state)?;
                let things = self.things.as_ref().ok_or("no thing table")?;
                crate::replay::anchor_mc2(&mut self.world, &self.pristine, things, &img, &st, t)?;
            }
        }
        let n = pool.ents.len();
        let human = pool.human as usize;
        if human != 0 && human < n {
            let he = pool.ents[human];
            let nx = he.next as usize;
            let sound = nx == 0
                || (nx < n
                    && pool.ents[nx].class != 0
                    && pool.ents[nx].linked
                    && !pool.ents[nx].ghost
                    && pool.ents[nx].cell == he.cell);
            if he.class != 0 && he.linked && sound {
                let (pc, pn) = self.world.player_chain_shadow();
                if pc != he.cell as usize || pn != he.next {
                    self.seat.hit(t, (he.class, he.model), || {
                        format!(
                            "t={t}: retail's carpet in cell {} successor {}, the imported seat \
                             cell {pc} successor {pn}",
                            he.cell, he.next
                        )
                    });
                }
            }
        }
        // Retail's chain with the human spliced out — the port keeps
        // the carpet out of the pool.
        let chain_of = |head: u16| -> Vec<u16> {
            let mut out = Vec::new();
            let (mut cur, mut steps) = (head as usize, 0);
            while cur != 0 && cur < n && steps <= n {
                if cur != human {
                    out.push(cur as u16);
                }
                cur = pool.ents[cur].next as usize;
                steps += 1;
            }
            out
        };
        for (cell, &h) in idx.iter().enumerate() {
            let want = chain_of(h);
            let got = self.world.map_chain_cell(cell);
            if want == got {
                continue;
            }
            let live: Vec<u16> = want
                .iter()
                .copied()
                .filter(|&s| !pool.ents[s as usize].ghost)
                .collect();
            if live == got {
                let g = want
                    .iter()
                    .copied()
                    .find(|&s| pool.ents[s as usize].ghost)
                    .unwrap_or(0) as usize;
                self.ghost
                    .hit(t, (pool.ents[g].class, pool.ents[g].model), || {
                        format!(
                            "t={t} cell {cell}: retail chain {want:?} (slot {g} disabled), port \
                             chain {got:?}"
                        )
                    });
                continue;
            }
            let (wh, gh) = (
                want.first().copied().unwrap_or(0),
                got.first().copied().unwrap_or(0),
            );
            if wh != gh {
                let who = if wh != 0 { wh } else { gh } as usize;
                let cm = pool.ents.get(who).map_or((0, 0), |e| (e.class, e.model));
                self.heads.hit(t, cm, || {
                    format!(
                        "t={t} cell {cell}: retail chain {want:?} (captured head {h}), port chain \
                         {got:?}"
                    )
                });
            } else if want != got {
                let cm = pool
                    .ents
                    .get(wh as usize)
                    .map_or((0, 0), |e| (e.class, e.model));
                self.order.hit(t, cm, || {
                    format!("t={t} cell {cell}: retail chain {want:?}, port chain {got:?}")
                });
            }
        }
        Ok(())
    }

    fn render(&self) -> Vec<String> {
        [
            ("IMPORT retail links a DISABLED record, the port does not", &self.ghost),
            ("IMPORT head differs", &self.heads),
            ("IMPORT chain differs under one head", &self.order),
            ("IMPORT human seat differs", &self.seat),
        ]
        .iter()
        .filter_map(|(n, t)| t.render(n))
        .collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn init_block(
    path: &std::path::Path,
    args: &Args,
    name: &str,
    family: Family,
    game: &str,
    level: u32,
    planes: &[String],
    cells: usize,
    init: Option<&mgc_formats::mgcr::InitRecord>,
    first: &mgc_formats::mgcr::TickRecord,
    first_planes: Option<&[Vec<u8>]>,
    first_index: Option<&[u16]>,
    rank: &[u16],
) -> Result<(), String> {
    let Some(init) = init else {
        println!("LANE {name} INIT: NONE — no init record");
        return Ok(());
    };
    let istate = init
        .tick
        .state
        .as_ref()
        .ok_or("the init record carries no state")?;
    let ipool = pool_of(family, istate)?;
    let fpool = pool_of(family, first.state.as_ref().ok_or("record 0 has no state")?)?;

    // -- retail: init → record 0 -----------------------------------------
    let occ = |p: &Pool| -> BTreeMap<u16, (u8, u8)> {
        p.ents
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, e)| e.class != 0)
            .map(|(s, e)| (s as u16, (e.class, e.model)))
            .collect()
    };
    let (io, fo) = (occ(&ipool), occ(&fpool));
    let mut born: BTreeMap<(u8, u8), u32> = BTreeMap::new();
    let mut gone: BTreeMap<(u8, u8), u32> = BTreeMap::new();
    let mut changed = 0u32;
    for (s, cm) in &fo {
        match io.get(s) {
            None => *born.entry(*cm).or_default() += 1,
            Some(o) if o != cm => changed += 1,
            _ => {}
        }
    }
    for (s, cm) in &io {
        if !fo.contains_key(s) {
            *gone.entry(*cm).or_default() += 1;
        }
    }
    let fmt = |m: &BTreeMap<(u8, u8), u32>| -> String {
        if m.is_empty() {
            "none".into()
        } else {
            m.iter()
                .map(|((c, k), n)| format!("({c},{k})×{n}"))
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    println!(
        "  INIT → RECORD 0 (retail, frame 1{}): pool {} → {} live · born {} · gone {} · \
         re-classed {changed}",
        if first.t == 0 {
            String::new()
        } else {
            format!(", record 0 is t={}", first.t)
        },
        io.len(),
        fo.len(),
        fmt(&born),
        fmt(&gone)
    );
    println!(
        "    rand {:#010x} → {:#010x} ({}) · terrain_rand {:?} → {:?} ({})",
        ipool.rand,
        fpool.rand,
        if ipool.rand == fpool.rand {
            "still"
        } else {
            "DRAWN"
        },
        init.tick.terrain_rand,
        first.terrain_rand,
        match (init.tick.terrain_rand, first.terrain_rand) {
            (Some(a), Some(b)) => format!(
                "{} draw(s)",
                rank[b as usize].wrapping_sub(rank[a as usize])
            ),
            _ => "—".into(),
        }
    );
    let iblob = init.terrain.as_deref();
    let mut frame1_cells = Vec::new();
    if let (Some(blob), Some(fp)) = (iblob, first_planes) {
        for (i, p) in planes.iter().enumerate() {
            let a = &blob[i * cells..(i + 1) * cells];
            let d = a.iter().zip(&fp[i]).filter(|(x, y)| x != y).count();
            frame1_cells.push(format!("{p} {d}"));
        }
        println!(
            "    planes changed by frame 1 (cells): {}",
            frame1_cells.join(" · ")
        );
    }
    // The init record's own index against its own pool.
    let iidx = iblob.and_then(|b| index_of_blob(planes, cells, b));
    let mut icensus = IndexCensus::default();
    if let Some(idx) = &iidx {
        icensus.walk(0, idx, &ipool);
        for (nm, tl) in [
            ("init: FREE record in a chain", &icensus.dead_member),
            ("init: link bit clear in a chain", &icensus.unlinked_member),
            ("init: member stands on another cell", &icensus.wrong_cell),
            ("init: back link broken", &icensus.prev_break),
            ("init: linked live record no chain reaches", &icensus.orphan),
            ("init: RULE captured head, none derived", &icensus.rule_cap_only),
            ("init: RULE derived head, none captured", &icensus.rule_der_only),
            ("init: RULE heads differ", &icensus.rule_differ),
        ] {
            if let Some(l) = tl.render(nm) {
                println!("{l}");
            }
        }
    }

    // -- the port's native build -----------------------------------------
    // Record 0's world (the settle `init-check` uses) and the settle-0
    // world, which is what stands beside the init record.
    let settle0 = match args.settle {
        Some(n) => n,
        None => crate::record0_settle(path, first, family, game, level, args)?.0,
    };
    let (w_rec0, p_rec0) =
        crate::native_settled_world(path, first, family, game, level, args, settle0)?;
    let (w_init, p_init) = if settle0 == 0 {
        (None, p_rec0.clone())
    } else {
        let (w, p) = crate::native_settled_world(path, first, family, game, level, args, 0)?;
        (Some(w), p)
    };
    let w_init = w_init.as_ref().unwrap_or(&w_rec0);

    let port_plane = |p: &mgc_sim::engine::features::Planes, name: &str| -> Option<Vec<u8>> {
        match name {
            "height" => Some(p.height.clone()),
            "type" => Some(p.tile_type.clone()),
            "shading" => Some(p.shading.clone()),
            "angle" => Some(p.angle.clone()),
            "ceiling" if !p.ceiling.is_empty() => Some(p.ceiling.clone()),
            _ => None,
        }
    };
    let plane_diff = |truth: &dyn Fn(usize) -> Option<Vec<u8>>,
                      port: &mgc_sim::engine::features::Planes|
     -> (u64, Vec<String>) {
        let mut total = 0u64;
        let mut out = Vec::new();
        for (i, p) in planes.iter().enumerate() {
            let (Some(a), Some(b)) = (truth(i), port_plane(port, p)) else {
                continue;
            };
            let d = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            let bits = a.iter().zip(&b).fold(0u8, |m, (x, y)| m | (x ^ y));
            total += d as u64;
            out.push(if d == 0 {
                format!("{p} 0")
            } else {
                format!("{p} {d} (bits {bits:#04x})")
            });
        }
        (total, out)
    };
    let (init_cells, init_rows) = plane_diff(
        &|i| iblob.map(|b| b[i * cells..(i + 1) * cells].to_vec()),
        &p_init,
    );
    let (rec0_cells, rec0_rows) = plane_diff(&|i| first_planes.map(|fp| fp[i].clone()), &p_rec0);
    println!(
        "  PORT terrain vs INIT (settle 0): {}",
        init_rows.join(" · ")
    );
    println!(
        "  PORT terrain vs RECORD 0 (settle {settle0}): {}",
        rec0_rows.join(" · ")
    );

    // terrain_rand against the port's retile LCG.
    let (pi, p0) = (w_init.pseudo_state(), w_rec0.pseudo_state());
    let say = |want: Option<u16>, got: u16| match want {
        Some(w) if w == got => format!("retail {w} port {got} MATCH"),
        Some(w) => format!(
            "retail {w} port {got} DIFFERENT (port is {} draw(s) ahead)",
            rank[got as usize].wrapping_sub(rank[w as usize]) as i16
        ),
        None => "—".into(),
    };
    println!(
        "  PORT terrain_rand vs INIT: {} · vs RECORD 0: {}",
        say(init.tick.terrain_rand, pi),
        say(first.terrain_rand, p0)
    );
    // MC2: retail's own retile table beside the one the port generates.
    let retile = init.building_f2cd0.as_deref().map(|truth| {
        let port = w_init.retile_bytes();
        let d = truth
            .iter()
            .zip(&port)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i, *a, *b))
            .collect::<Vec<_>>();
        println!(
            "  PORT retile table vs INIT building_F2CD0x: retail {} B, port {} B, {} byte(s) \
             differ{}",
            truth.len(),
            port.len(),
            d.len(),
            d.first().map_or(String::new(), |(i, a, b)| format!(
                "  e.g. row {} byte {}: retail {a:#04x} port {b:#04x}",
                i / 2,
                i % 2
            ))
        );
        d.len() + truth.len().abs_diff(port.len())
    });
    let trand_init = init.tick.terrain_rand == Some(pi);
    let trand_rec0 = first.terrain_rand == Some(p0);

    // The native head table against the captured one. The port's carpet
    // is out of the pool, so retail's chain is read with the human
    // spliced out.
    let head_diff = |idx: Option<&[u16]>,
                     pool: &Pool,
                     w: &mgc_sim::engine::world::World,
                     only_live_in_pool: bool,
                     tag: &str|
     -> (u64, u64) {
        let Some(idx) = idx else { return (0, 0) };
        let n = pool.ents.len();
        let human = pool.human as usize;
        let (mut heads, mut order) = (Tally::default(), Tally::default());
        for (cell, &h) in idx.iter().enumerate() {
            let mut want = Vec::new();
            let (mut cur, mut steps) = (h as usize, 0);
            while cur != 0 && cur < n && steps <= n {
                if cur != human {
                    want.push(cur as u16);
                }
                cur = pool.ents[cur].next as usize;
                steps += 1;
            }
            // Beside the INIT record the port's build already holds
            // frame 1's births (carpets, tokens, rival castles): take
            // every record retail's pool does not hold out of the
            // port's chain, so the row is about the records both have.
            let got: Vec<u16> = w
                .map_chain_cell(cell)
                .into_iter()
                .filter(|&s| {
                    !only_live_in_pool || pool.ents.get(s as usize).is_some_and(|e| e.class != 0)
                })
                .collect();
            let (wh, gh) = (
                want.first().copied().unwrap_or(0),
                got.first().copied().unwrap_or(0),
            );
            let who = if wh != 0 { wh } else { gh } as usize;
            let cm = pool.ents.get(who).map_or((0, 0), |e| (e.class, e.model));
            let cm = if cm.0 == 0 {
                w.debug_pool_rows()
                    .iter()
                    .find(|r| r.0 == who)
                    .map_or((0, 0), |r| (r.1, r.2))
            } else {
                cm
            };
            if wh != gh {
                heads.hit(0, cm, || {
                    format!("cell {cell}: retail chain {want:?}, port chain {got:?}")
                });
            } else if want != got {
                order.hit(0, cm, || {
                    format!("cell {cell}: retail chain {want:?}, port chain {got:?}")
                });
            }
        }
        for (nm, tl) in [("head differs", &heads), ("chain differs", &order)] {
            if let Some(l) = tl.render(&format!("{tag} {nm}")) {
                println!("{l}");
            }
        }
        (heads.rows, order.rows)
    };
    let (ih, io_) = head_diff(iidx.as_deref(), &ipool, w_init, true, "PORT index vs INIT:");
    let (fh, fo_) = head_diff(first_index, &fpool, &w_rec0, false, "PORT index vs RECORD 0:");

    // The human's seat in the NATIVE build, at record 0 (he does not
    // exist before frame 1).
    let seat = {
        let h = fpool.human as usize;
        match fpool.ents.get(h).filter(|e| h != 0 && e.class != 0 && e.linked) {
            Some(e) => {
                let (pc, pn) = w_rec0.player_chain_shadow();
                let ok = pc == e.cell as usize && pn == e.next;
                println!(
                    "  PORT human seat vs RECORD 0: retail cell {} successor {}, port cell {} \
                     successor {pn} — {}",
                    e.cell,
                    e.next,
                    if pc == usize::MAX {
                        "UNSEEDED".to_string()
                    } else {
                        pc.to_string()
                    },
                    if ok { "MATCH" } else { "DIFFERENT" }
                );
                if ok { "MATCH" } else { "DIFF" }
            }
            None => "—",
        }
    };

    // The port's settle-0 pool beside the init record's.
    let port_occ: BTreeMap<u16, (u8, u8)> = w_init
        .debug_pool_rows()
        .into_iter()
        .map(|(s, c, m, ..)| (s as u16, (c, m)))
        .collect();
    let mut p_only: BTreeMap<(u8, u8), u32> = BTreeMap::new();
    let mut r_only: BTreeMap<(u8, u8), u32> = BTreeMap::new();
    let mut differ = 0u32;
    for (s, cm) in &port_occ {
        match io.get(s) {
            // Frame 1's own births are the port constructor's too.
            None if fo.get(s) == Some(cm) => {}
            None => *p_only.entry(*cm).or_default() += 1,
            Some(o) if o != cm => differ += 1,
            _ => {}
        }
    }
    for (s, cm) in &io {
        if !port_occ.contains_key(s) {
            *r_only.entry(*cm).or_default() += 1;
        }
    }
    println!(
        "  PORT pool (settle 0) vs INIT: port {} live, retail {} · port-only past frame 1's births \
         {} · retail-only {} · slot holds another record {differ}",
        port_occ.len(),
        io.len(),
        fmt(&p_only),
        fmt(&r_only)
    );
    println!(
        "LANE {name} INIT: frame 1 born {} gone {} rand {} trand {} · port terrain vs init {} \
         cell(s), vs record 0 {} · trand init {} rec0 {} · index init {}+{} rec0 {}+{} · pool \
         retail-only {} port-only {} differ {differ} · init index integrity {} rule {} · \
         retile table {} · human seat {seat}",
        born.values().sum::<u32>(),
        gone.values().sum::<u32>(),
        if ipool.rand == fpool.rand {
            "still"
        } else {
            "drawn"
        },
        match (init.tick.terrain_rand, first.terrain_rand) {
            (Some(a), Some(b)) => rank[b as usize]
                .wrapping_sub(rank[a as usize])
                .to_string(),
            _ => "—".into(),
        },
        init_cells,
        rec0_cells,
        if trand_init { "MATCH" } else { "DIFF" },
        if trand_rec0 { "MATCH" } else { "DIFF" },
        ih,
        io_,
        fh,
        fo_,
        r_only.values().sum::<u32>(),
        p_only.values().sum::<u32>(),
        icensus.rows(),
        icensus.rule_rows(),
        retile.map_or("—".to_string(), |n| n.to_string()),
    );
    Ok(())
}

/// `MGC_INDEX=1` — THE FREE RUN'S HEAD TABLE against the captured one
/// (`replay`): after every stepped tick, each cell's chain as the port
/// holds it beside retail's captured head walked through retail's own
/// links, the human spliced out (the port's carpet is out of the pool).
///
/// A row is sorted by what RETAIL's chain holds: `stale` = the chain
/// passes through a record that is FREE, unlinked or standing on
/// another cell, or either side's chain holds a record retail's table
/// MISPLACES that tick (linked, live, and reached from no head or from
/// another cell's) — retail's own table is inconsistent there, the
/// registered book-scatter deviation (docs/DEVIATIONS.md); `clean` =
/// retail's table is sound there and the port's chain differs.
pub(crate) struct IndexLane {
    on: bool,
    name: String,
    ticks: u64,
    clean_head: Tally,
    clean_order: Tally,
    stale: Tally,
    /// Retail links a DISABLED record (MC2) and the port's chain is
    /// retail's without it.
    ghost: Tally,
    /// A sound-chain row whose two chains hold the SAME records in
    /// another order, keyed by the first pair that stands swapped:
    /// (the record retail walks first, the record the port walks
    /// first).
    swapped: BTreeMap<((u8, u8), (u8, u8)), u64>,
    /// A sound-chain row whose two chains hold DIFFERENT records.
    membership: Tally,
    /// THE HUMAN'S SEAT: the port keeps his carpet out of the pool and
    /// carries his rank as `(cell, successor)`; retail's is his own
    /// record's cell and `next` link.
    seat_cell: Tally,
    seat_next: Tally,
    seat_n: u64,
}

impl IndexLane {
    pub(crate) fn new(path: &std::path::Path) -> Self {
        Self {
            on: std::env::var_os("MGC_INDEX").is_some(),
            name: crate::verify::take_stem(path),
            ticks: 0,
            clean_head: Tally::default(),
            clean_order: Tally::default(),
            stale: Tally::default(),
            ghost: Tally::default(),
            swapped: BTreeMap::new(),
            membership: Tally::default(),
            seat_cell: Tally::default(),
            seat_next: Tally::default(),
            seat_n: 0,
        }
    }

    pub(crate) fn emit(
        &mut self,
        world: &mgc_sim::engine::world::World,
        timg: &Option<TerrainImage>,
        pool: impl FnOnce() -> Pool,
        t: u64,
    ) {
        if !self.on {
            return;
        }
        let Some(idx) = timg.as_ref().filter(|i| i.based()).and_then(index_of) else {
            return;
        };
        let pool = pool();
        self.ticks += 1;
        let n = pool.ents.len();
        let human = pool.human as usize;
        if human != 0 && human < n {
            let he = pool.ents[human];
            // Only where retail's own table holds him: linked, and
            // reachable from the head of the cell he stands on.
            let mut cur = idx[he.cell as usize] as usize;
            let mut steps = 0;
            while cur != 0 && cur != human && cur < n && steps <= n {
                cur = pool.ents[cur].next as usize;
                steps += 1;
            }
            // His successor a record retail's table misplaces: the
            // book-scatter deviation, not a seat row.
            // …or a record retail's table has LOST from his cell (the
            // truncated chain of a scattered slot's tenant).
            let tainted = {
                let nx = he.next as usize;
                (nx != 0
                    && nx < n
                    && (pool.ents[nx].class == 0
                        || !pool.ents[nx].linked
                        || pool.ents[nx].cell != he.cell))
                    || {
                        let mut under = vec![false; n];
                        let (mut c, mut k) = (idx[he.cell as usize] as usize, 0);
                        while c != 0 && c < n && k <= n {
                            under[c] = true;
                            c = pool.ents[c].next as usize;
                            k += 1;
                        }
                        (1..n).any(|s| {
                            let e = pool.ents[s];
                            e.class != 0 && e.linked && e.cell == he.cell && !under[s]
                        })
                    }
            };
            if he.class != 0 && he.linked && cur == human && !tainted {
                self.seat_n += 1;
                let (pc, pn) = world.player_chain_shadow();
                let cm = pool
                    .ents
                    .get(he.next as usize)
                    .map_or((0, 0), |e| (e.class, e.model));
                if pc != he.cell as usize {
                    self.seat_cell.hit_loud(t, cm, || {
                        format!(
                            "t={t}: retail's carpet sits in cell {} ({},{}), the port's seat in {pc}",
                            he.cell,
                            he.cell & 0xFF,
                            he.cell >> 8
                        )
                    });
                } else if pn != he.next {
                    self.seat_next.hit_loud(t, cm, || {
                        format!(
                            "t={t} cell {} ({},{}): retail's successor {}, the port's {pn}",
                            he.cell,
                            he.cell & 0xFF,
                            he.cell >> 8,
                            he.next
                        )
                    });
                }
            }
        }
        // Which records retail's table MISPLACES this tick — live and
        // linked, and reached from no head or from the head of a cell
        // they do not stand on. Walked once, on the tick's first row.
        let mut misplaced: Option<Vec<bool>> = None;
        let misplaced_of = |idx: &[u16], pool: &Pool| -> Vec<bool> {
            let mut under = vec![usize::MAX; n];
            for (cell, &h) in idx.iter().enumerate() {
                let (mut cur, mut steps) = (h as usize, 0);
                while cur != 0 && cur < n && steps <= n {
                    if under[cur] == usize::MAX {
                        under[cur] = cell;
                    }
                    cur = pool.ents[cur].next as usize;
                    steps += 1;
                }
            }
            (0..n)
                .map(|s| {
                    let e = pool.ents[s];
                    s != 0 && e.class != 0 && e.linked && under[s] != e.cell as usize
                })
                .collect()
        };
        for (cell, &h) in idx.iter().enumerate() {
            let head = world.map_head_cell(cell);
            if h == 0 && head == 0 {
                continue;
            }
            let mut want = Vec::new();
            let mut sound = true;
            let (mut cur, mut steps) = (h as usize, 0);
            while cur != 0 && cur < n && steps <= n {
                let e = pool.ents[cur];
                if e.class == 0 || !e.linked || e.cell as usize != cell {
                    sound = false;
                }
                if cur != human {
                    want.push(cur as u16);
                }
                cur = e.next as usize;
                steps += 1;
            }
            let got = world.map_chain_cell(cell);
            if want == got {
                continue;
            }
            let (wh, gh) = (
                want.first().copied().unwrap_or(0),
                got.first().copied().unwrap_or(0),
            );
            let who = if wh != 0 { wh } else { gh } as usize;
            let cm = pool.ents.get(who).map_or((0, 0), |e| (e.class, e.model));
            let say = || {
                format!(
                    "t={t} cell {cell} ({},{}): retail chain {want:?}, port chain {got:?}",
                    cell & 0xFF,
                    cell >> 8
                )
            };
            // A chain the broken table truncated is sound to look at:
            // the records it lost are the witnesses.
            let sound = sound && {
                let m = misplaced.get_or_insert_with(|| misplaced_of(&idx, &pool));
                !want
                    .iter()
                    .chain(&got)
                    .any(|&s| m.get(s as usize).copied().unwrap_or(false))
            };
            if !sound {
                self.stale.hit(t, cm, say);
                continue;
            }
            let live: Vec<u16> = want
                .iter()
                .copied()
                .filter(|&s| !pool.ents[s as usize].ghost)
                .collect();
            if live == got {
                self.ghost.hit(t, cm, say);
                continue;
            }
            if std::env::var_os("MGC_INDEX_VERBOSE").is_some() {
                println!("INDEXROW {}", say());
            }
            let (mut a, mut b) = (want.clone(), got.clone());
            a.sort_unstable();
            b.sort_unstable();
            if a == b {
                if let Some((x, y)) = want.iter().zip(&got).find(|(x, y)| x != y) {
                    let of = |s: u16| pool.ents.get(s as usize).map_or((0, 0), |e| (e.class, e.model));
                    *self.swapped.entry((of(*x), of(*y))).or_default() += 1;
                }
            } else {
                self.membership.hit(t, cm, say);
            }
            if wh != gh {
                self.clean_head.hit(t, cm, say);
            } else {
                self.clean_order.hit(t, cm, say);
            }
        }
    }
}

impl Drop for IndexLane {
    fn drop(&mut self) {
        if !self.on {
            return;
        }
        for (nm, tl) in [
            ("INDEX head differs (retail's chain sound)", &self.clean_head),
            ("INDEX order differs (retail's chain sound)", &self.clean_order),
            ("INDEX retail's own chain is unsound", &self.stale),
            ("INDEX retail links a disabled record", &self.ghost),
            ("INDEX MEMBERSHIP differs (retail's chain sound)", &self.membership),
            ("INDEX human seat: CELL differs", &self.seat_cell),
            ("INDEX human seat: SUCCESSOR differs", &self.seat_next),
        ] {
            if let Some(l) = tl.render(nm) {
                println!("{l}");
            }
        }
        if !self.swapped.is_empty() {
            let mut v: Vec<_> = self.swapped.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            let v: Vec<String> = v
                .iter()
                .take(12)
                .map(|(((a, b), (c, d)), n)| format!("({a},{b})<({c},{d})×{n}"))
                .collect();
            println!("    INDEX SWAPS (retail-first<port-first): {}", v.join(" "));
        }
        println!(
            "INDEX {}: {} — {} stepped tick(s) · head {} row(s) on {} tick(s) · order {} on {} · \
             retail-unsound {} on {} · disabled-record {} on {} · membership {} on {} · human seat \
             {} cell + {} successor row(s) of {}",
            self.name,
            if self.ticks == 0 {
                "NONE"
            } else if self.clean_head.rows
                + self.clean_order.rows
                + self.seat_cell.rows
                + self.seat_next.rows
                == 0
            {
                "IDENTICAL"
            } else {
                "ROWS"
            },
            self.ticks,
            self.clean_head.rows,
            self.clean_head.ticks,
            self.clean_order.rows,
            self.clean_order.ticks,
            self.stale.rows,
            self.stale.ticks,
            self.ghost.rows,
            self.ghost.ticks,
            self.membership.rows,
            self.membership.ticks,
            self.seat_cell.rows,
            self.seat_next.rows,
            self.seat_n
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(ents: &[(usize, Link)]) -> Pool {
        let mut v = vec![Link::default(); 16];
        for (s, l) in ents {
            v[*s] = *l;
        }
        Pool {
            ents: v,
            human: 0,
            rand: 0,
        }
    }

    fn live(cell: u16, next: u16, prev: u16) -> Link {
        Link {
            class: 10,
            model: 0,
            linked: true,
            ghost: false,
            next,
            prev,
            cell,
        }
    }

    #[test]
    fn the_lcg_rank_counts_draws() {
        let rank = lcg_rank();
        let mut x = 21632u16;
        let from = x;
        for k in 0..1000u16 {
            assert_eq!(rank[x as usize].wrapping_sub(rank[from as usize]), k);
            x = x.wrapping_mul(9377).wrapping_add(9439);
        }
        // Full period: every state has its own rank.
        let mut seen = vec![false; 65536];
        for r in &rank {
            assert!(!std::mem::replace(&mut seen[*r as usize], true));
        }
    }

    #[test]
    fn a_sound_table_walks_clean() {
        // Cell 5: 3 -> 7; cell 9: 4.
        let p = pool(&[(3, live(5, 7, 0)), (7, live(5, 0, 3)), (4, live(9, 0, 0))]);
        let mut idx = vec![0u16; 65536];
        idx[5] = 3;
        idx[9] = 4;
        let mut c = IndexCensus::default();
        c.walk(1, &idx, &p);
        assert_eq!((c.heads, c.members), (2, 3));
        assert_eq!(c.rows() + c.rule_rows(), 0);
    }

    #[test]
    fn a_free_record_at_a_head_is_a_row_and_no_rule_derives_it() {
        // The book scatter's shape: cell 5 is headed by FREE slot 3,
        // which still points at the live token 7.
        let free = Link {
            class: 0,
            linked: true,
            next: 7,
            cell: 5,
            ..Link::default()
        };
        let p = pool(&[(3, free), (7, live(5, 0, 3))]);
        let mut idx = vec![0u16; 65536];
        idx[5] = 3;
        let mut c = IndexCensus::default();
        c.walk(1, &idx, &p);
        assert_eq!(c.dead_member.rows, 1);
        assert_eq!(c.rule_cap_only.rows, 1);
        assert_eq!(c.orphan.rows, 0);
    }

    #[test]
    fn a_tenant_of_a_scattered_slot_hangs_under_two_cells() {
        // Slot 3 was linked under cell 5 while free and is now a live
        // record standing on cell 9, head of that chain too.
        let p = pool(&[(3, live(9, 0, 0)), (7, live(5, 0, 3))]);
        let mut idx = vec![0u16; 65536];
        idx[5] = 3;
        idx[9] = 3;
        let mut c = IndexCensus::default();
        c.walk(1, &idx, &p);
        assert_eq!(c.wrong_cell.rows, 1);
        assert_eq!(c.shared.rows, 1);
        // 7 is linked and live and no chain reaches it.
        assert_eq!(c.orphan.rows, 1);
    }
}
