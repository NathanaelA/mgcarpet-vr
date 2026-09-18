//! THE RAW SHADOW — the ungraded-lane detector (`MGC_RAW_SHADOW=1`).
//!
//! `EntObsMc1` carries 22 fields; `RetailEntMc1` carries 50-odd. Every
//! field in the gap is a lane the recording HOLDS, the importer
//! RESTORES and the graded diff can never see, so a handler that reads
//! one correctly and writes it wrong reports CLEAN in pair mode
//! forever. The lane started as `+70`/`+71`/`+58`/`+44` and each of the
//! four paid for itself (the frozen `+58` combat gate, the kraken's
//! `+71` burst counter, the crater's inherited `+44`); this module is
//! that comparison widened to every field the port models, and pointed
//! at BOTH runners:
//!
//! - **pair mode** (`verify-deltas`) imports retail state every tick,
//!   so a mismatch is a one-tick WRITE bug, attributable to the
//!   handler that ran;
//! - **the free run** (`replay`) carries its own copy for thousands of
//!   ticks, so a mismatch is where the port's own history first parts
//!   from retail's — which is the only instrument that can explain a
//!   segmented-replay break whose pair diff `D1(t)` is CLEAN.
//!
//! Run both: pair mode names the guilty handler, the free run names the
//! tick that mattered.

use mgc_formats::mgcr::{RetailMc1, RetailMc2};
use mgc_sim::engine::world::World;
use mgc_sim::engine::world::conformance::{
    norm_retail_ai_state_mc1, norm_retail_ai_state_mc2, retail_ent_lanes_mc2,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

/// The port's out-of-pool human sentinel as a slot number.
const PLAYER_TARGET_U16: u16 = u16::MAX;

/// The MC2 raw lanes the graded diff reports UNCONDITIONALLY, so the
/// shadow must not double-count: the seventeen `EntObsMc2` carries,
/// plus `f5a` — the sprite lane, graded off the raw channel since
/// session 68 (`append_sprite_diffs_mc2`). See [`mc2_lane_graded`]
/// for the three conditional ones.
const GRADED_MC2_ALWAYS: [&str; 18] = [
    "f5a",
    "class3f",
    "model40",
    "life",
    "max_life",
    "x",
    "y",
    "z",
    "pitch",
    "ayaw",
    "apitch",
    "speed",
    "mana",
    "mana_max",
    "action45",
    "sv1",
    "player_ent",
    "rand",
];

/// Is this raw lane already visible to `compare_mc2_gated`?
///
/// ⚠ THREE OF THE TWENTY GRADED LANES ARE GRADED ONLY CONDITIONALLY,
/// and a flat name list blinds the census exactly where they are NOT
/// graded (the trap an adversarial verifier caught in the first draft
/// of this module):
///
/// - `yaw` (@0x1C → obs `heading`) is SKIPPED for class 15 by
///   `compare_mc2_gated` — manifestations repurpose the world-yaw lane
///   for the subSpellIndex payload — so raw `yaw` is UNGRADED there.
/// - obs `sv2` projects `site_z` for class 5 and a hard `0` otherwise
///   (`obs_project_mc2`), so raw `sv2` carries no graded information
///   off class 5.
/// - obs `owner` projects through the FOUR translated families
///   (class-15 manifestation, (10,42) painter, (5,10) pyramid, and the
///   pyramid-summoned (5,{0,19,21,25})) and a hard `0` everywhere else,
///   so raw `owner28` is ungraded on every other record.
///
/// ⚠ The `owner28` test is deliberately the CONSERVATIVE half of
/// `obs_project_mc2`'s predicate: that one additionally requires
/// `id24 != slot` and, for the class-5 summons, that the referenced
/// record really is a live (5,10). Reproducing it here would mean
/// re-deriving the entity graph inside the instrument, so a WILD
/// (5,0) multipart body — whose owner lane retail projects as 0, i.e.
/// ungraded — is over-skipped. Bounded and documented; run with
/// `MGC_RAW_SHADOW_ALL=1` to drop every exclusion and see it.
fn mc2_lane_graded(name: &str, class: u8, model: u8) -> bool {
    if GRADED_MC2_ALWAYS.contains(&name) {
        return true;
    }
    match name {
        "yaw" => class != 15,
        "sv2" => class == 5,
        "owner28" => {
            class == 15
                || (class == 10 && model == 42)
                || (class == 5 && matches!(model, 10 | 0 | 19 | 21 | 25))
        }
        _ => false,
    }
}

/// The six damage mailboxes as lane names — `{amount, source}` per
/// channel (ch0 physical, ch1 mana-ball claim, ch3 mana steal, ch4
/// grip, ch5 balloon recall).
const MAIL_LANES: [(&str, &str); 6] = [
    ("mail0.amt", "mail0.src"),
    ("mail1.amt", "mail1.src"),
    ("mail2.amt", "mail2.src"),
    ("mail3.amt", "mail3.src"),
    ("mail4.amt", "mail4.src"),
    ("mail5.amt", "mail5.src"),
];

/// One lane's running verdict: how many rows, the FIRST one, the
/// tick span and the DISTINCT slot census ("45 rows across 1 slot"
/// and "45 rows across 40 slots" are completely different leads).
#[derive(Default, Clone)]
pub(crate) struct Lane {
    pub(crate) rows: u64,
    pub(crate) first_t: u64,
    pub(crate) last_t: u64,
    pub(crate) slots: std::collections::BTreeSet<u16>,
    pub(crate) example: String,
}

/// The tally across a run. Keyed `(class, model, field)` because one
/// key is one story — a lane that is wrong on `(10,23)` and right on
/// `(10,25)` is a different bug from one that is wrong on both.
#[derive(Default)]
pub(crate) struct Shadow {
    pub(crate) lanes: BTreeMap<(u8, u8, &'static str), Lane>,
    /// The WIZEXT half (`World::wiz_shadow_mc1`), keyed `(wiz, field)`
    /// — the Lane's slot census holds the ARRAY INDEX there (0 for
    /// scalars).
    pub(crate) wiz_lanes: BTreeMap<(u8, &'static str), Lane>,
    /// Free-stack verdict: mismatched pairs / compared pairs / first
    /// example.
    pub(crate) free: (u64, u64, String),
    /// The MC2 RECYCLE stack's own verdict — MC2 pops free first and
    /// falls back to recycle, so the two stacks are separate
    /// allocators and a single merged compare would measure the
    /// importer's COMPOSITION rather than the port's order.
    pub(crate) recycle: (u64, u64, String),
    /// THE OBJECTIVE BOARD (`struct_0x3659C[local]`), keyed
    /// `(lane, row)` — row 0 for the three scalars. See
    /// [`Self::compare_board_mc2`].
    pub(crate) board: BTreeMap<(&'static str, u8), Lane>,
    /// Set once [`Self::compare_board_mc2`] has run, for the same
    /// reason as `wiz_fed`: "0 mismatches" and "nobody looked" must
    /// never print the same.
    pub(crate) board_fed: bool,
    /// Boundaries at which the board was compared (the denominator).
    pub(crate) board_n: u64,
    /// Set once [`Self::compare_wiz_mc1`] or [`Self::compare_wiz_mc2`]
    /// has run — a silent "0 mismatches" would read as "the player
    /// block is clean" when it means "nobody looked", which is exactly
    /// how MC2's half hid until it was built.
    pub(crate) wiz_fed: bool,
    /// Per-wizard DENOMINATOR: boundaries whose block was compared /
    /// skipped (eliminated in retail, or seated in a different slot).
    /// A rival skipped on every boundary reads exactly like a clean
    /// one without it.
    pub(crate) wiz_n: BTreeMap<u8, (u64, u64)>,
    /// Retail lane names absent from the port's table (`?` in
    /// `dump-state`): a table skew, reported once rather than per slot.
    pub(crate) skew: BTreeSet<&'static str>,
    /// ⚠⚠ **THE SHADOW'S OWN BLIND SPOT, MADE VISIBLE** (round 149).
    /// [`Self::compare_ents_mc2`] skips every slot `torn_slots` named,
    /// and said nothing about it — so a family whose `phase3e` steps
    /// by a legal non-1 amount (the SWARM/multipart bodies) is never
    /// shadowed on ANY take and the summary still prints a confident
    /// mismatch count. "Nobody looked" and "clean" must never render
    /// the same, which is the rule `wiz_fed`/`board_fed` already
    /// encode; this is the per-entity half of it.
    ///
    /// Keyed `(class, model)` of the RETAIL record, value = slot
    /// exclusions (one per slot per boundary).
    pub(crate) hidden: BTreeMap<(u8, u8), u64>,
    /// Distinct slots the tear gate hid at least once.
    pub(crate) hidden_slots: BTreeSet<u16>,
    /// Boundaries on which `compare_ents_mc2` ran — the DENOMINATOR
    /// for the exclusion count (a "1,000 exclusions" headline means
    /// nothing without the number of pairs it is spread over).
    pub(crate) ent_n: u64,
    /// `MGC_RAW_SHADOW_ALL=1` — report every lane, graded or not.
    pub(crate) all_lanes: bool,
    /// The world under comparison is a NATIVE build (`init-check`),
    /// whose owned MC1 tokens carry the port's own `MANIFEST_BASE +
    /// spell` in +70; an imported world carries retail's `3·spell`
    /// and a `MANIFEST_BASE` token THERE is a real divergence (the
    /// two mc1l0 cheat takes: the human's token mint ignores the
    /// strict encoding), so the normalization is scoped to this.
    pub(crate) native: bool,
    /// The world under comparison is a PINNED-POSE PAIR (`verify-deltas`):
    /// `exec_pair` ticks the human with `drive = None`, so the carpet
    /// MOVER never runs — and four human lanes are the mover's own:
    /// `knock_mag` (the knock bleeds 4/tick in `take_knock_step` — the
    /// round-147 "+4 on 40/40 takes" artifact, the same shape round 154
    /// proved on MC1's 38/38), `duel_count` / `duel_hold` (`sub_455D0`
    /// :55249 counts the lock in the mover tail; w154g: 934 rows on
    /// mc1l48, ZERO in the free run) and `duel_victim` (the release
    /// test lives there too). Those rows measure the HARNESS, not the
    /// port, so the pair comparator skips them (⚖ the player, round
    /// 154); the FREE run (`replay`) keeps all four.
    pub(crate) pair_pinned: bool,
    /// The UNMODELLED census's memory (`<lane>~` rows): the last retail
    /// value per (owner, lane, idx), so a row counts a TRANSITION, not
    /// a tick — a rival's hands are `0/1` on every tick of every take,
    /// and "400 rows" of that says nothing; "changed twice" does.
    pub(crate) unmodelled_last: BTreeMap<(u16, &'static str, u16), i64>,
    /// The bucket-chain denominator: (links compared, links skipped as
    /// severed/unlisted) — "0 chain0 rows" must say how many links it
    /// stands on.
    pub(crate) chain_n: (u64, u64),
    /// `MGC_ALLOC_CENSUS=1` — print one classified line per
    /// mismatching allocator boundary (see [`Self::alloc_census`]).
    alloc_census: usize,
    /// Optional per-row TSV (`MGC_RAW_SHADOW_ROWS=<path>`). Wizard
    /// rows ride the same file with class 255, model = wiz, slot =
    /// array index.
    rows: Option<std::io::BufWriter<std::fs::File>>,
    /// `MGC_RAW_SHADOW_LANE=<class>,<model>,<field>` — the lane
    /// magnifier: every row of exactly that lane prints to stdout as
    /// it lands, so the census line's one `e.g.` widens to the whole
    /// story without a TSV round-trip.
    watch: Option<(u8, u8, String)>,
    /// `MGC_WIZ_SHADOW_LANE=<wiz>,<field>` — the wizard-lane twin of
    /// the magnifier.
    wiz_watch: Option<(u8, String)>,
}

impl Shadow {
    /// The tally `init-check` feeds: EVERY lane, graded or not — a
    /// native world was never imported, so no lane is "already
    /// judged" — and none of the env-driven magnifiers.
    pub(crate) fn census_all() -> Self {
        // …except the per-row TSV, which costs nothing when unset and
        // is the only way to read a census lane's WHOLE row list
        // (`init-check` prints one `e.g.` per lane).
        let rows = std::env::var_os("MGC_RAW_SHADOW_ROWS").and_then(|p| {
            let mut w = std::io::BufWriter::new(std::fs::File::create(&p).ok()?);
            writeln!(w, "t\tslot\tclass\tmodel\tfield\tretail\tport").ok()?;
            Some(w)
        });
        Shadow {
            all_lanes: true,
            native: true,
            rows,
            ..Default::default()
        }
    }

    /// The GRADED core of an MC1 record, for `init-check` only.
    /// [`Self::compare_ents_mc1`] walks the curated UNGRADED lanes
    /// (with the importer's re-homes mirrored); the pair runners leave
    /// the rest to the obs diff, which a native world has no pinned
    /// projection for. These are the lanes of the two dump-state
    /// tables that carry straight across with no re-home.
    pub(crate) fn compare_core_mc1(&mut self, world: &World, st: &RetailMc1, human_slot: u16, t: u64) {
        const CORE: [&str; 9] = [
            "rand", "max_life", "act_life", "flags", "x", "y", "z", "type86", "f63",
        ];
        for (slot, re) in st.ents.iter().enumerate() {
            let slot = slot as u16;
            if slot == 0 || slot == human_slot || re.class64 == 0 {
                continue;
            }
            let Some(port) = world.port_ent_lanes_mc1(slot, human_slot, false) else {
                continue;
            };
            let port: BTreeMap<&'static str, Option<i64>> = port.into_iter().collect();
            let same = matches!(port.get("class64"), Some(Some(c)) if *c == re.class64 as i64)
                && matches!(port.get("model65"), Some(Some(m)) if *m == re.model65 as i64);
            if !same {
                continue;
            }
            for (name, rv) in mgc_sim::engine::world::conformance::retail_ent_lanes_mc1(re) {
                if !CORE.contains(&name) {
                    continue;
                }
                if let Some(Some(pv)) = port.get(name)
                    && *pv != rv
                {
                    self.hit((re.class64, re.model65, name), t, slot, rv, *pv);
                }
            }
        }
    }

    /// Build the tally if `MGC_RAW_SHADOW` is set, opening the row TSV
    /// if `MGC_RAW_SHADOW_ROWS` names one. `None` = the instrument is
    /// off and costs nothing.
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        if std::env::var_os("MGC_RAW_SHADOW").is_none() {
            return Ok(None);
        }
        // Player-ruled 2026-09-18: the (10,23)/(10,38) null-victim
        // heap ghost (44546, `mc2_null_victim_ghost`) is an INSTRUMENT
        // default — the census stamps it so the `target96` lane reads
        // honest; the sim and the graded runners never do.
        // `MGC_MC2_NULL_VICTIM_GHOST=0` overrides.
        mgc_sim::mc2::set_null_victim_ghost_instrument(true);
        let rows = match std::env::var_os("MGC_RAW_SHADOW_ROWS") {
            Some(p) => {
                let mut w = std::io::BufWriter::new(
                    std::fs::File::create(&p).map_err(|e| format!("shadow rows: {e}"))?,
                );
                writeln!(w, "t\tslot\tclass\tmodel\tfield\tretail\tport")
                    .map_err(|e| format!("shadow rows: {e}"))?;
                Some(w)
            }
            None => None,
        };
        let watch = match std::env::var("MGC_RAW_SHADOW_LANE") {
            Ok(v) => {
                let mut it = v.splitn(3, ',');
                match (it.next(), it.next(), it.next()) {
                    (Some(c), Some(m), Some(f)) => {
                        let c = c
                            .trim()
                            .parse()
                            .map_err(|_| format!("MGC_RAW_SHADOW_LANE: bad class in {v:?}"))?;
                        let m = m
                            .trim()
                            .parse()
                            .map_err(|_| format!("MGC_RAW_SHADOW_LANE: bad model in {v:?}"))?;
                        Some((c, m, f.trim().to_string()))
                    }
                    _ => {
                        return Err(format!(
                            "MGC_RAW_SHADOW_LANE={v:?}: want <class>,<model>,<field>"
                        ));
                    }
                }
            }
            Err(_) => None,
        };
        let wiz_watch = match std::env::var("MGC_WIZ_SHADOW_LANE") {
            Ok(v) => {
                let mut it = v.splitn(2, ',');
                match (it.next(), it.next()) {
                    (Some(w), Some(f)) => {
                        let w = w
                            .trim()
                            .parse()
                            .map_err(|_| format!("MGC_WIZ_SHADOW_LANE: bad wiz in {v:?}"))?;
                        Some((w, f.trim().to_string()))
                    }
                    _ => return Err(format!("MGC_WIZ_SHADOW_LANE={v:?}: want <wiz>,<field>")),
                }
            }
            Err(_) => None,
        };
        Ok(Some(Shadow {
            rows,
            watch,
            wiz_watch,
            all_lanes: std::env::var_os("MGC_RAW_SHADOW_ALL").is_some(),
            alloc_census: match std::env::var("MGC_ALLOC_CENSUS") {
                Ok(v) => v.trim().parse().unwrap_or(4),
                Err(_) => 0,
            },
            ..Default::default()
        }))
    }

    fn hit(&mut self, key: (u8, u8, &'static str), t: u64, slot: u16, a: i64, b: i64) {
        let lane = self.lanes.entry(key).or_default();
        lane.rows += 1;
        if lane.example.is_empty() {
            lane.first_t = t;
            lane.example = format!("t={t} slot {slot}: retail {a} port {b}");
        }
        lane.last_t = t;
        lane.slots.insert(slot);
        if let Some(w) = self.rows.as_mut() {
            let _ = writeln!(w, "{t}\t{slot}\t{}\t{}\t{}\t{a}\t{b}", key.0, key.1, key.2);
        }
        if let Some((wc, wm, wf)) = self.watch.as_ref()
            && *wc == key.0
            && *wm == key.1
            && wf == key.2
        {
            println!(
                "  LANE ({},{}) {} t={t} slot {slot}: retail {a} port {b}",
                key.0, key.1, key.2
            );
        }
    }

    /// The wizard-lane twin of [`Self::hit`]; `idx` is the array index
    /// (0 for scalars).
    fn wiz_hit(&mut self, key: (u8, &'static str), t: u64, idx: u16, a: i64, b: i64) {
        let lane = self.wiz_lanes.entry(key).or_default();
        lane.rows += 1;
        if lane.example.is_empty() {
            lane.first_t = t;
            lane.example = format!("t={t} wiz {} [{idx}]: retail {a} port {b}", key.0);
        }
        lane.last_t = t;
        lane.slots.insert(idx);
        if let Some(w) = self.rows.as_mut() {
            let _ = writeln!(w, "{t}\t{idx}\t255\t{}\t{}\t{a}\t{b}", key.0, key.1);
        }
        if let Some((ww, wf)) = self.wiz_watch.as_ref()
            && *ww == key.0
            && wf == key.1
        {
            println!(
                "  WIZ LANE {} {} t={t} [{idx}]: retail {a} port {b}",
                key.0, key.1
            );
        }
    }

    /// The board-lane twin of [`Self::hit`]; `row` is the objective
    /// row index (0 for the three scalars). TSV class 254 (wizard
    /// rows use 255), model = the row index.
    fn board_hit(&mut self, name: &'static str, row: u8, t: u64, a: i64, b: i64) {
        let lane = self.board.entry((name, row)).or_default();
        lane.rows += 1;
        if lane.example.is_empty() {
            lane.first_t = t;
            lane.example = format!("t={t}: retail {a} port {b}");
        }
        lane.last_t = t;
        lane.slots.insert(row as u16);
        if let Some(w) = self.rows.as_mut() {
            let _ = writeln!(w, "{t}\t{row}\t254\t{row}\t{name}\t{a}\t{b}");
        }
    }

    /// ⭐⭐⭐ **THE OBJECTIVE BOARD IS RECORDED BUT WAS UNGRADED** —
    /// exactly the class this module exists for, and it hid mc2l22's
    /// t=1200 wall for two campaign rounds (round 98, dig Q13: a
    /// phantom type-2 completion at t=1199 fired the m32 switch's
    /// disposition, whose `sub_49F90` rebuild + 50 spawns WAS the
    /// wall). The bytes were always in the capture
    /// (`RetailMc2::objectives[8][11]`, `mgcr.rs:2419`,
    /// `type_substr_3659C` / LevelStructs.h:190-196, stride 11):
    ///
    /// - `[0]` `IsLevelEnd_0`   ← the port's `World::completed()`
    /// - `[1]` `ObjectiveText_1` — the CURRENT-row cursor
    /// - `[2]` `ObjectiveDone_2` — the m32 one-pass pause
    /// - `[3..11]` `stage_0x3659F[8]` — per-row state (1 active, 2 done)
    ///
    /// `sub_58F00_game_objectives` (EF:40693) reads and writes them at
    /// the FRAME TAIL and the class-11 model-32 switches gate on
    /// `stage_0x3659F[par1] == 2` (EF:54369), so a board row that
    /// latches one tick early detonates the pool on the NEXT walk.
    /// Only the LOCAL player's board is compared — `mc2_stages` is the
    /// only board the port models.
    ///
    /// ⚠ WHICH LANE IS SPEAKING MATTERS HERE MORE THAN ANYWHERE. In
    /// PAIR mode the importer RESTORES all four lanes from retail@t
    /// (`world/conformance.rs:2658`) before the tick runs, so a row is
    /// a genuine ONE-TICK write bug attributable to the handler that
    /// ran. In the FREE RUN the board has been the port's own since
    /// the anchor, so the first row is where the port's own objective
    /// history parts — which is the lane the horizon measures.
    ///
    /// ⚠ `mc2_stages` is COMPACTED by the baker (`set_mc2_stages`), so
    /// port index == retail row by construction (the importer relies
    /// on the same identity). Retail's row-state bytes past the port's
    /// stage count are still checked — against 0 — rather than
    /// silently dropped, because an absence there would be a baker
    /// law, not a clean lane.
    pub(crate) fn compare_board_mc2(&mut self, world: &World, st: &RetailMc2, t: u64) {
        let Some(board) = st.objectives.get(st.local_player as usize) else {
            return;
        };
        self.board_fed = true;
        self.board_n += 1;
        let (cur, rows) = world.mc2_objective_view();
        for (name, want, got) in [
            ("completed", board[0] as i64, world.completed() as i64),
            ("cursor", board[1] as i64, cur as i64),
            ("pause", board[2] as i64, world.mc2_objective_pause() as i64),
        ] {
            if want != got {
                self.board_hit(name, 0, t, want, got);
            }
        }
        for k in 0..8usize {
            let want = board[3 + k] as i64;
            // A row the port does not model publishes 0 — retail's own
            // value for an unauthored row, and the only honest thing to
            // say about a real one.
            let got = rows.get(k).map_or(0, |r| r.1 as i64);
            if want != got {
                self.board_hit("state", k as u8, t, want, got);
            }
        }
        // The row COUNT itself: a port board shorter or longer than
        // retail's authored set is a loader story, not a latch story,
        // and would otherwise show up as a flood of `state` rows.
        let want_n = board[3..].iter().filter(|&&s| s != 0).count() as i64;
        let got_n = rows.iter().filter(|r| r.1 != 0).count() as i64;
        if want_n != got_n {
            self.board_hit("live_rows", 0, t, want_n, got_n);
        }
    }

    /// Diff every ungraded per-entity lane of `world` against the
    /// recorded state `st`, at tick `t`.
    ///
    /// Only slots the recording agrees are the SAME entity are
    /// compared — a slot desync would otherwise report every field on
    /// every shifted slot and drown the lane it is meant to expose.
    pub(crate) fn compare_ents_mc1(
        &mut self,
        world: &World,
        st: &RetailMc1,
        human_slot: u16,
        t: u64,
    ) {
        for g in world.raw_shadow_mc1() {
            if g.slot == human_slot {
                continue;
            }
            let Some(w) = st.ents.get(g.slot as usize) else {
                continue;
            };
            if w.class64 != g.class || w.model65 != g.model {
                continue;
            }
            // Lanes that hold a POOL SLOT need the obs projection's own
            // untranslation: the port carries the human as
            // `PLAYER_TARGET` (0xFFFF) because its carpet is not a pool
            // record, where the recording carries the real slot.
            // Without it every latch, claim and mail source naming the
            // human reads as a mismatch — 400k rows of it on mc1l2, all
            // of them the sentinel.
            let untr = |v: i64| {
                if v == u16::MAX as i64 {
                    human_slot as i64
                } else {
                    v
                }
            };
            // ⭐⭐ MIRROR THE IMPORTER'S RE-HOMES OR THE LANE IS A LIE.
            // `import_ent` (crates/mgc-sim/src/engine/world/
            // conformance.rs:5580-99) does NOT carry three retail
            // fields straight across, so the port's field and the
            // recording's same-named field are different quantities:
            //   * a class-12 token's burst/refire counter lives at
            //     retail +48 and the port keeps it in `f26` (retail's
            //     own +26 is the spell LEVEL there);
            //   * the same token's OWNER carpet slot lives at retail
            //     +42 with +144 ALWAYS 0, and the port stamps it into
            //     `f144`;
            //   * a (10,41) castle leveler's "current rung" moves from
            //     retail +48 into the port's `f28`.
            // Comparing the port's re-homed field against the RAW
            // recording lane fires on every such record on every tick:
            // 4,548 of 4,839 entity rows — 94% — of a 23-tick mc1l49
            // raw-shadow report were exactly this, and the noise HID
            // the +48 lane the shadow exists to watch (no "f48" pair
            // is listed below because the port has no `f48` at all).
            // `MGC_RAW_SHADOW_RAW_LANES=1` restores the pre-2026-09-07
            // raw comparison.
            let (rf26, rf28, rf144) = if std::env::var_os("MGC_RAW_SHADOW_RAW_LANES").is_some() {
                (w.f26 as i64, w.f28 as i64, w.f144 as i64)
            } else {
                (
                    if w.class64 == 12 {
                        w.f48 as i64
                    } else {
                        w.f26 as i64
                    },
                    if w.class64 == 10 && w.model65 == 41 {
                        w.f48 as i64
                    } else {
                        w.f28 as i64
                    },
                    if w.class64 == 12 && w.f144 == 0 {
                        w.f42 as i64
                    } else {
                        w.f144 as i64
                    },
                )
            };
            // A NATIVE world's owned token carries `MANIFEST_BASE +
            // spell` in +70 where retail (and every imported world)
            // carries `3·spell + phase`, phase 0 = owned — the port's
            // own encoding, not a lane (`init-check` is the only
            // native-world caller; 38 of 39 MC1 takes fired on it).
            // Scoped to `self.native`: see the field.
            // …and an AUTHORED jar carries the bare phase (`0..=2`,
            // `spawn_from_thing_at`'s native `base = 0`) where retail
            // carries `3·spell + phase` (round 154, w154k: the
            // `(12,x) f70` "retail 34 port 1" rows on mc1l10/l13/l16/
            // l20/l21/hwl2 — one per authored jar, all exactly this).
            let gf70 = if self.native
                && g.class == 12
                && g.f70 >= mgc_sim::engine::world::MANIFEST_BASE
            {
                3 * (g.f70 - mgc_sim::engine::world::MANIFEST_BASE) as i64
            } else if self.native && g.class == 12 && g.f70 <= 2 {
                3 * g.model as i64 + g.f70 as i64
            } else {
                g.f70 as i64
            };
            let mut hits: Vec<(&'static str, i64, i64)> = vec![
                ("f70", w.f70 as i64, gf70),
                ("f71", w.f71 as i64, g.f71 as i64),
                // ⚠ COMPARE THE BYTE, NOT THE NUMBER. The recording's
                // `+58` is signed and the port widens it to i16, but
                // the port's own decrements wrap as u8 — retail's -6
                // and the port's 250 are the SAME byte and only the
                // sign interpretation differs. Masking keeps this lane
                // honest; the sign question is its own lead.
                ("f58", w.f58 as i64 & 0xFF, g.f58 as i64 & 0xFF),
                ("f44", w.f44 as i64, g.f44 as i64),
                ("f26", rf26, g.f26 as i64),
                ("f28", rf28, g.f28 as i64),
                ("f36", w.f36 as i64, g.f36 as i64),
                ("f38", w.f38 as i64, untr(g.f38 as i64)),
                ("f40", w.f40 as i64, untr(g.f40 as i64)),
                ("f46", w.f46 as i64, g.f46 as i64),
                ("f50", w.f50 as i64, g.f50 as i64),
                ("f52", w.f52 as i64, g.f52 as i64),
                ("f54", w.f54 as i64, g.f54 as i64),
                ("f56", w.f56 as i64, g.f56 as i64),
                // ⭐ A (3,2) CASTLE'S `f59` IS NOT RETAIL'S `+59`: the
                // importer derives it from the transform machine's
                // `+48` sub-state (`import_ent`: `+70 == 5` →
                // `{1,4} → 1, s → min(s, 6)`, else 0). Round 153's
                // first census compared the raw byte and read 136,975
                // rows on 39/39 takes of exactly that mis-home.
                (
                    "f59",
                    if w.class64 == 3 && w.model65 == 2 {
                        if w.f70 == 5 {
                            match w.f48 {
                                1 | 4 => 1,
                                s => (s as u8).min(6) as i64,
                            }
                        } else {
                            0
                        }
                    } else {
                        w.f59 as i64
                    },
                    g.f59 as i64,
                ),
                ("f66", w.f66 as i64, g.f66 as i64),
                ("f67", w.f67 as i64, g.f67 as i64),
                ("f68", w.f68 as i64, g.f68 as i64),
                ("f69", w.f69 as i64, g.f69 as i64),
                ("f78", w.f78 as i64, g.f78 as i64),
                ("f80", w.f80 as i64, g.f80 as i64),
                ("f82", w.f82 as i64, g.f82 as i64),
                ("f84", w.f84 as i64, g.f84 as i64),
                // type86 GRADED since session 68 (append_sprite_diffs)
                // — dropped here so the shadow doesn't double-count.
                ("frame88", w.frame88 as i64, g.frame88 as i64),
                ("frames89", w.frames89 as i64, g.frames89 as i64),
                ("f128", w.f128 as i64, g.f128 as i64),
                ("f130", w.f130 as i64, g.f130 as i64),
                ("f144", rf144, untr(g.f144 as i64)),
                ("dest_x", w.dest_x as i64, g.dest_x as i64),
                ("dest_y", w.dest_y as i64, g.dest_y as i64),
                ("site_z", w.site_z as i64, g.site_z as i64),
            ];
            for (k, (amt, src)) in MAIL_LANES.iter().enumerate() {
                hits.push((amt, w.mail[k].0 as i64, g.mail[k].0 as i64));
                hits.push((src, w.mail[k].1 as i64, untr(g.mail[k].1 as i64)));
            }
            // Retail's `+48` raw (`Ent::raw48`), everywhere a re-homed
            // copy is not already compared under its own name.
            let rehomed48 = w.class64 == 12
                || (w.class64 == 10 && w.model65 == 41)
                || (w.class64 == 3 && w.model65 == 2);
            if !rehomed48 {
                hits.push(("f48", w.f48 as i64, g.f48 as i64));
            }
            // The castle workers' `+42` castle link (`Ent::link42`,
            // w154j): a compared lane on the (10,41) leveler and the
            // (10,42) painter — retail's slot vs the port's stamp.
            if w.class64 == 10 && matches!(w.model65, 41 | 42) {
                hits.push(("f42", w.f42 as i64, g.f42 as i64));
            }
            // The TILE LINKS are structural, not a lane: the port's
            // carpet lives outside the pool, so a chain that threads
            // THROUGH the human can never agree link-for-link. Skip
            // exactly those rows and keep the rest — chain ORDER is
            // where every membership law this campaign has found landed
            // first.
            for (name, a, b) in [
                ("next20", w.next20 as i64, g.next20 as i64),
                ("prev22", w.prev22 as i64, g.prev22 as i64),
            ] {
                if a != human_slot as i64 {
                    hits.push((name, a, b));
                }
            }
            for (name, a, b) in hits {
                if a != b {
                    self.hit((g.class, g.model, name), t, g.slot, a, b);
                }
            }
            // ⚠ THE UNMODELLED CENSUS (round 153): lanes the port has
            // NO home for, so nothing can be compared — but a retail
            // value that is ever NON-ZERO says the lane carries state
            // the port never will. Reported under `<lane>~` with the
            // port side printed as `—` (−1); `+42` off class 12 (the
            // token owner, homed in `f144`) and off (10,41)/(10,42) (the
            // castle link, compared above since w154j).
            let worker = w.class64 == 10 && matches!(w.model65, 41 | 42);
            let unmodelled: [(&'static str, i64); 3] = [
                ("f42~", if w.class64 == 12 || worker { 0 } else { w.f42 as i64 }),
                ("f61~", w.f61 as i64),
                ("f62~", w.f62 as i64),
            ];
            for (name, a) in unmodelled {
                let last = self.unmodelled_last.insert((g.slot, name, 0), a);
                if a != 0 && last != Some(a) {
                    self.hit((g.class, g.model, name), t, g.slot, a, -1);
                }
            }
        }
    }

    /// Diff every ungraded WIZEXT/brain lane against the recording's
    /// wizard slice — the Type_160 half of the shadow. Pair mode names
    /// the handler that wrote a register wrong THIS tick; the free run
    /// names the first tick the port's carried wizard state parts from
    /// retail's, which is the only instrument that can explain a
    /// carpet-motion break whose pair diff is CLEAN (the mc1l4 t=5378
    /// family this was built for).
    /// The MC2 twin of [`Self::compare_ents_mc1`] — and a NAME-CHECKED
    /// ZIP of the two lane tables, not a second field map.
    ///
    /// ⭐ `retail_ent_lanes_mc2` (conformance.rs) and
    /// `World::port_ent_lanes_mc2` are 91 lanes, name-for-name AND
    /// order-for-order identical, and are ALREADY the retail↔port join
    /// that `dump-state --port` renders. Reusing them is the whole
    /// reason this arm is short: a hand-rolled second table is exactly
    /// the failure mode this campaign keeps finding ("ONE ladder, THREE
    /// copies, TWO wrong"). `None` on the port side means the port does
    /// not model the lane for this record's class — the `—` column —
    /// and is skipped, never counted as a mismatch.
    ///
    /// Two gates the MC1 arm does not need:
    /// - **TORN SLOTS.** `verify_mc2::torn_slots` drops any slot whose
    ///   `phase3e` moved by an amount its SPECIES' CADENCE cannot
    ///   produce — a per-entity capture tear, one pass early or late.
    ///   ⚠ Round 149 narrowed that test: a null-dispatch row HOLDS its
    ///   phase byte and a birth RE-SEEDS it, and neither is a tear.
    ///   `compare_mc2_gated` already skips them, so counting them here
    ///   would report the capture, not the port.
    /// - **CLASS/MODEL AGREEMENT**, read off the port's own table: a
    ///   slot holding a different entity on the two sides is a
    ///   missing/extra story the graded diff owns.
    /// THE TILE-CHAIN HEAD TABLE (`mapEntityIndex_15B4E0`), compared cell
    /// by cell. Retail's table is not recorded, but it is exactly
    /// derivable from the record: every linked entity (`flags & 4`) whose
    /// `prev18` is 0 heads the chain of its own cell `(x >> 8, y >> 8)` —
    /// measured on whole takes (mc2l4, mc2l1: one head per cell, no link
    /// crossing cells, 2026-09-14). The per-entity link rows above cannot
    /// see a chain hung under the WRONG cell when the entity is alone in
    /// it (next16 == prev18 == 0 either way), and the tile scans can, so
    /// this is the one structural lane the link rows leave open.
    /// Rows land on lane `(class, model, "map_head")` of the retail head
    /// (or of the port's stray head when retail has none): retail = the
    /// expected head slot, port = the port's. The human is out-of-pool on
    /// the port side (its rank is carried by `player_chain`), so a cell either
    /// side heads with the carpet is skipped; torn slots too.
    pub(crate) fn compare_map_heads_mc2(
        &mut self,
        world: &World,
        st: &RetailMc2,
        human_slot: u16,
        torn: &BTreeSet<u16>,
        t: u64,
    ) {
        let mut expect = vec![0u16; 65536];
        for (slot, re) in st.ents.iter().enumerate() {
            if slot == 0 || re.class3f == 0 || re.flags & 4 == 0 || re.prev18 != 0 {
                continue;
            }
            let cell = ((re.y >> 8) as usize) << 8 | (re.x >> 8) as usize;
            expect[cell] = slot as u16;
        }
        for (cell, &w) in expect.iter().enumerate() {
            let g = world.map_head_cell(cell);
            if w == g || w == human_slot || g == human_slot || torn.contains(&w) || torn.contains(&g) {
                continue;
            }
            let re = &st.ents[if w != 0 { w } else { g } as usize];
            self.hit((re.class3f, re.model40, "map_head"), t, if w != 0 { w } else { g }, w as i64, g as i64);
        }
    }

    pub(crate) fn compare_ents_mc2(
        &mut self,
        world: &World,
        st: &RetailMc2,
        human_slot: u16,
        torn: &BTreeSet<u16>,
        t: u64,
    ) {
        self.ent_n += 1;
        for (slot, re) in st.ents.iter().enumerate() {
            let slot = slot as u16;
            if slot == 0 || slot == human_slot || re.class3f == 0 {
                continue;
            }
            // ⚠ THE TEAR GATE IS AN EXCLUSION — COUNT IT. See
            // [`Shadow::hidden`]. This slot is about to be dropped
            // from every ungraded lane; without this tally the
            // summary's "0 mismatches" on a swarm family is
            // indistinguishable from "never looked at".
            if torn.contains(&slot) {
                *self.hidden.entry((re.class3f, re.model40)).or_insert(0) += 1;
                self.hidden_slots.insert(slot);
                continue;
            }
            let Some(port) = world.port_ent_lanes_mc2(slot, human_slot, false) else {
                continue;
            };
            let port: BTreeMap<&'static str, Option<i64>> = port.into_iter().collect();
            // The port's own class/model, read from the table it just
            // handed us — no second source of truth.
            let same = matches!(port.get("class3f"), Some(Some(c)) if *c == re.class3f as i64)
                && matches!(port.get("model40"), Some(Some(m)) if *m == re.model40 as i64);
            if !same {
                continue;
            }
            for (name, rv) in retail_ent_lanes_mc2(re) {
                if !self.all_lanes && mc2_lane_graded(name, re.class3f, re.model40) {
                    continue;
                }
                // THE TILE LINKS are structural, not a lane: the port's
                // carpet lives outside the pool, so a chain threading
                // THROUGH the human can never agree link-for-link.
                // Skip exactly those rows (MC1 does the same for
                // next20/prev22) and keep the rest — chain ORDER is
                // where every membership law this campaign has found
                // landed first.
                if matches!(name, "next16" | "prev18") && rv == human_slot as i64 {
                    continue;
                }
                match port.get(name) {
                    // The port models it and disagrees.
                    Some(Some(pv)) if *pv != rv => {
                        self.hit((re.class3f, re.model40, name), t, slot, rv, *pv);
                    }
                    Some(_) => {}
                    // A lane in the retail table with no port twin —
                    // the two tables have skewed. Report ONCE (the
                    // render prints a ⚠ line) instead of flooding.
                    None => {
                        self.skew.insert(name);
                    }
                }
            }
        }
    }

    /// The MC2 allocator twin of [`Self::compare_free_mc1`]. Both
    /// stacks, separately — MC2 pops FREE first and falls back to
    /// RECYCLE, so merging them would measure the importer's
    /// composition rather than the port's order. Filtered exactly the
    /// way `dump-state --port` filters them, so a difference is the
    /// port's own allocator.
    pub(crate) fn compare_free_mc2(
        &mut self,
        world: &World,
        st: &RetailMc2,
        human_slot: u16,
        t: u64,
    ) {
        let pool = st.ents.len();
        let keep = |s: &u16| (*s as usize) < pool && *s != human_slot;
        let want_free: Vec<u16> = st.free_stack.iter().copied().filter(keep).collect();
        let want_rec: Vec<u16> = st.recycle_stack.iter().copied().filter(keep).collect();
        let (got_free, got_rec) = world.free_stacks_mc2();
        for (want, got, acc) in [
            (want_free, got_free, &mut self.free),
            (want_rec, got_rec, &mut self.recycle),
        ] {
            acc.1 += 1;
            if want != got {
                acc.0 += 1;
                if acc.2.is_empty() {
                    // Depth from the TOP is what matters: the next
                    // spawn pops the end, so depth 0 diverging is a
                    // slot handed out wrong THIS tick.
                    let depth = want
                        .iter()
                        .rev()
                        .zip(got.iter().rev())
                        .position(|(a, b)| a != b);
                    acc.2 = format!(
                        "t={t} len retail {} port {}, top retail {:?} port {:?}, \
                         first top-diff depth {}",
                        want.len(),
                        got.len(),
                        want.last(),
                        got.last(),
                        depth.map_or("none (prefix)".into(), |d| d.to_string()),
                    );
                }
            }
        }
        self.alloc_census(world, st, human_slot, t);
    }

    /// `MGC_ALLOC_CENSUS=1` — the per-boundary CLASSIFIER for the two
    /// allocator stacks (round 98, dig Q23). The census line only ever
    /// carried its FIRST example, which cannot tell an ORDER
    /// difference from a DEPTH one from a MEMBERSHIP one — and those
    /// are three different bugs. Default-OFF, print-only; it changes
    /// nothing the grader accepts.
    ///
    /// One line per mismatching boundary:
    /// `ACENSUS t=<t> <stack> kind=<K> rlen=<n> plen=<m> depth=<d>
    ///  onlyR=[..] onlyP=[..]` where `kind` is
    /// `MEMBER` (the two sets differ), `ORDER` (same set, different
    /// order) or `LEN` (one is a proper prefix of the other, top
    /// aligned).
    fn alloc_census(&mut self, world: &World, st: &RetailMc2, human_slot: u16, t: u64) {
        let dep = self.alloc_census;
        if dep == 0 {
            return;
        }
        let pool = st.ents.len();
        let keep = |s: &u16| (*s as usize) < pool && *s != human_slot;
        let (pf, pr) = world.free_stacks_mc2();
        for (name, want, got) in [
            (
                "free",
                st.free_stack
                    .iter()
                    .copied()
                    .filter(keep)
                    .collect::<Vec<_>>(),
                pf.to_vec(),
            ),
            (
                "recycle",
                st.recycle_stack
                    .iter()
                    .copied()
                    .filter(keep)
                    .collect::<Vec<_>>(),
                pr.to_vec(),
            ),
        ] {
            if want == got {
                continue;
            }
            let ws: BTreeSet<u16> = want.iter().copied().collect();
            let gs: BTreeSet<u16> = got.iter().copied().collect();
            let only_r: Vec<u16> = ws.difference(&gs).copied().collect();
            let only_p: Vec<u16> = gs.difference(&ws).copied().collect();
            let depth = want
                .iter()
                .rev()
                .zip(got.iter().rev())
                .position(|(a, b)| a != b);
            let kind = if !only_r.is_empty() || !only_p.is_empty() {
                "MEMBER"
            } else if want.len() == got.len() {
                "ORDER"
            } else {
                "LEN"
            };
            let cm = |s: u16| -> String {
                let r = st
                    .ents
                    .get(s as usize)
                    .map_or("?".into(), |e| format!("({},{})", e.class3f, e.model40));
                // ⭐ dig 98-Q29: retail's class alone cannot tell "the
                // port dropped a spawn" from "the port kept a record
                // retail freed" — both print as a MEMBER difference.
                // The PORT's class/model at the same slot separates
                // them in one glance. Print-only, like the rest.
                let p =
                    world
                        .port_ent_lanes_mc2(s, human_slot, false)
                        .map_or("-".to_string(), |v| {
                            let m: BTreeMap<&'static str, Option<i64>> = v.into_iter().collect();
                            format!(
                                "({},{})",
                                m.get("class3f").copied().flatten().unwrap_or(-1),
                                m.get("model40").copied().flatten().unwrap_or(-1),
                            )
                        });
                format!("{s}:r{r}p{p}")
            };
            println!(
                "ACENSUS t={t} {name} kind={kind} rlen={} plen={} depth={} \
                 rtop={:?} ptop={:?} onlyR=[{}] onlyP=[{}]",
                want.len(),
                got.len(),
                depth.map_or("-1".into(), |d| d.to_string()),
                want.iter().rev().take(dep).collect::<Vec<_>>(),
                got.iter().rev().take(dep).collect::<Vec<_>>(),
                only_r
                    .iter()
                    .take(8)
                    .map(|&s| cm(s))
                    .collect::<Vec<_>>()
                    .join(" "),
                only_p
                    .iter()
                    .take(8)
                    .map(|&s| cm(s))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
    }

    pub(crate) fn compare_wiz_mc1(&mut self, world: &World, st: &RetailMc1, t: u64) {
        self.wiz_fed = true;
        for ws in world.wiz_shadow_mc1() {
            let Some(w) = st.wizards.get(ws.wiz as usize) else {
                continue;
            };
            // Eliminated on either side, or a carpet-slot desync: the
            // roster/graded comparison owns those stories.
            if w.play_index == 0 || (ws.wiz != 0 && w.play_index != ws.ent) {
                self.wiz_n.entry(ws.wiz).or_default().1 += 1;
                continue;
            }
            self.wiz_n.entry(ws.wiz).or_default().0 += 1;
            let ent = st.ents.get(w.play_index as usize);
            for &(name, port) in &ws.scalars {
                // The pinned pair's mover-owned human lanes (see the field).
                if self.pair_pinned
                    && ws.wiz == 0
                    && matches!(name, "knock_mag" | "duel_count" | "duel_hold" | "duel_victim")
                {
                    continue;
                }
                let retail: i64 = match name {
                    "charge" => w.charge as i64,
                    "knock_dir" => w.knock_dir as i64,
                    "knock_mag" => w.knock_mag as i64,
                    "danger" => w.danger as i64,
                    "aggro" => w.aggro as i64,
                    "banked_houses" => w.banked_houses as i64,
                    "castle_alert" => w.castle_alert as i64,
                    "player_alert" => w.player_alert as i64,
                    "balloon_alert" => w.balloon_alert as i64,
                    "kills" => w.kills as i64,
                    "shots" => w.shots as i64,
                    "hits" => w.hits as i64,
                    "cmd_speed" => w.cmd_speed as i64,
                    "strafe" => w.strafe as i64,
                    "grace" => w.grace as i64,
                    "regen_stall" => w.regen_stall as i64,
                    "life_rate" => w.life_rate as i64,
                    "ai_state" => norm_retail_ai_state_mc1(w.ai_state),
                    "burst" => w.burst as i64,
                    "poverty" => (w.poverty != 0) as i64,
                    // Round 153's widening (the MC1 half of round 147's
                    // MC2 table). The port's pool SCAN against retail's
                    // stored +50 — the register itself is graded.
                    "castle_scan" => w.castle as i64,
                    "tempo" => w.tempo as i64,
                    "ai_flag" => (w.ai_flag == 1) as i64,
                    "win_streak" => w.win_streak as i64,
                    "duel_victim" => w.duel_victim as i64,
                    // count/hold are stale once the lock drops; the
                    // port's register is an Option and reads 0.
                    "duel_count" | "duel_hold" if w.duel_victim == 0 => continue,
                    "duel_count" => w.duel_count as i64,
                    "duel_hold" => w.duel_hold as i64,
                    // The port's human-target sentinel is not retail's
                    // computed sig; those rows have no retail twin.
                    "target_sig" if port == u16::MAX as i64 => continue,
                    "target_sig" => match ent {
                        Some(e) => e.f148 as i64,
                        None => continue,
                    },
                    "mana_delta" => match ent {
                        Some(e) => e.f132 as i64,
                        None => continue,
                    },
                    _ => continue,
                };
                if retail != port {
                    self.wiz_hit((ws.wiz, name), t, 0, retail, port);
                }
            }
            for (name, port) in &ws.arrays {
                let retail: Vec<i64> = match *name {
                    "hate" => w.hate.iter().map(|&v| v as i64).collect(),
                    "war" => w.war.iter().map(|&v| (v != 0) as i64).collect(),
                    "learn" => w.learn.iter().map(|&v| v as i64).collect(),
                    "cooldown" => w.cooldown.iter().map(|&v| v as i64).collect(),
                    "owned" => w.owned_slots.iter().map(|&v| v as i64).collect(),
                    "acq" => w.spell_list.iter().map(|&v| v as i64).collect(),
                    "balloon_reg" => w.balloon_reg.iter().map(|&v| v as i64).collect(),
                    "guard_reg" => w.guard_reg.iter().map(|&v| v as i64).collect(),
                    _ => continue,
                };
                // Retail's length rules, the port padded with 0 (the
                // MC2 twin's rule since round 147): an owner the port
                // holds NO register for must read as all-empty, not
                // as "nothing to compare".
                for (i, &a) in retail.iter().enumerate() {
                    let b = port.get(i).copied().unwrap_or(0);
                    if a != b {
                        self.wiz_hit((ws.wiz, name), t, i as u16, a, b);
                    }
                }
            }
            // ⚠ THE UNMODELLED CENSUS (round 153) — see the entity twin
            // in [`Self::compare_ents_mc1`]. The registers retail keeps
            // per wizard that the port has NO home for on this column:
            // rivals' HUD alarms, kill/shot/hit counters,
            // raw hands (255 = empty) and blue-grant flags; the HUMAN's
            // hate ledger (0x601F = neutral), war flags, learn and
            // cooldown countdowns; everyone's exit-status word.
            let neutral_hate = 0x601F_i64;
            let mut unmodelled: Vec<(&'static str, u16, i64)> = vec![("status~", 0, w.status as i64)];
            if ws.wiz == 0 {
                for (i, &h) in w.hate.iter().enumerate() {
                    unmodelled.push(("hate~", i as u16, if h as i64 == neutral_hate { 0 } else { h as i64 }));
                }
                for (i, &v) in w.war.iter().enumerate() {
                    unmodelled.push(("war~", i as u16, v as i64));
                }
                for (i, &v) in w.learn.iter().enumerate() {
                    unmodelled.push(("learn~", i as u16, v as i64));
                }
                for (i, &v) in w.cooldown.iter().enumerate() {
                    unmodelled.push(("cooldown~", i as u16, v as i64));
                }
            } else {
                unmodelled.extend([
                    ("danger~", 0, w.danger as i64),
                    ("kills~", 0, w.kills as i64),
                    ("shots~", 0, w.shots as i64),
                    ("hits~", 0, w.hits as i64),
                    ("castle_alert~", 0, w.castle_alert as i64),
                    ("player_alert~", 0, w.player_alert as i64),
                    ("balloon_alert~", 0, w.balloon_alert as i64),
                    ("hand_left~", 0, if w.hand_left == 255 || w.hand_left == 0xFFFF { 0 } else { w.hand_left as i64 + 1 }),
                    ("hand_right~", 0, if w.hand_right == 255 || w.hand_right == 0xFFFF { 0 } else { w.hand_right as i64 + 1 }),
                ]);
                for (i, &v) in w.blue.iter().enumerate() {
                    unmodelled.push(("blue~", i as u16, v as i64));
                }
            }
            for (name, idx, a) in unmodelled {
                let last = self.unmodelled_last.insert((0x100 | ws.wiz as u16, name, idx), a);
                if a != 0 && last != Some(a) {
                    self.wiz_hit((ws.wiz, name), t, idx, a, -1);
                }
            }
        }
    }

    /// The MC2 twin of [`Self::compare_wiz_mc1`] — the player-block
    /// arm that did not exist before, so every rival brain register
    /// (state, burst, the 26 recast cooldowns, the hate ledger, the
    /// weave/steer FSMs, the charge meter, the stored castle index)
    /// rode 40,000 ticks of mc2l6 ungraded.
    pub(crate) fn compare_wiz_mc2(&mut self, world: &World, st: &RetailMc2, t: u64) {
        self.wiz_fed = true;
        for ws in world.wiz_shadow_mc2() {
            let Some(p) = st.players.get(ws.wiz as usize) else {
                continue;
            };
            // Eliminated on either side, or a carpet-slot desync: the
            // roster/graded comparison owns those stories.
            if p.play_index == 0 || (ws.wiz != 0 && p.play_index != ws.ent) {
                self.wiz_n.entry(ws.wiz).or_default().1 += 1;
                continue;
            }
            self.wiz_n.entry(ws.wiz).or_default().0 += 1;
            for &(name, port) in &ws.scalars {
                // The pinned pair's mover-owned human lanes (see the field).
                if self.pair_pinned
                    && ws.wiz == 0
                    && matches!(name, "knock_mag" | "duel_count" | "duel_hold" | "duel_victim")
                {
                    continue;
                }
                let retail: i64 = match name {
                    "charge" => p.charge as i64,
                    "cmd_speed" => p.cmd_speed as i64,
                    "strafe" => p.strafe as i64,
                    "brake" => (p.brake != 0) as i64,
                    "invuln" => p.invuln as i64,
                    "ai_state" => norm_retail_ai_state_mc2(p.ai_state),
                    "burst" => p.burst as i64,
                    "poverty" => (p.poverty != 0) as i64,
                    "aggression" => p.aggression as i64,
                    "perception" => p.perception as i64,
                    "reflexes" => p.reflexes as i64,
                    "life_scale" => p.life_scale as i64,
                    "weave" => p.weave as i64,
                    "weave_dir" => p.weave_dir as i64,
                    "avoid" => p.avoid as i64,
                    "avoid_exit" => p.avoid_exit as i64,
                    "castle_ent" => p.castle_ent as i64,
                    "hand_left" => p.hand_left as i64,
                    "hand_right" => p.hand_right as i64,
                    "life_regen" => p.life_regen as i64,
                    "knock_dir" => p.knock_dir as i64,
                    "knock_mag" => p.knock_mag as i64,
                    // The port keeps the stall in a u16 (the importer
                    // clamps the same way).
                    "regen_stall" => p.regen_stall.clamp(0, u16::MAX as i32) as i64,
                    // Retail's timer idles at <= 0; the port's rival
                    // map only holds live (> 0) entries.
                    "wanted" if ws.wiz != 0 => p.wanted.max(0) as i64,
                    "wanted" => p.wanted as i64,
                    "recast_surcharge" => (p.recast_surcharge != 0) as i64,
                    "duel_target" => p.duel_target as i64,
                    // hold/tier are stale once the lock drops; the
                    // port's register is an Option and reads 0.
                    "duel_hold" if p.duel_target == 0 => continue,
                    "duel_tier" if p.duel_target == 0 => continue,
                    "duel_hold" => p.duel_hold as i64,
                    "duel_tier" => p.duel_tier as i64,
                    _ => continue,
                };
                if retail != port {
                    self.wiz_hit((ws.wiz, name), t, 0, retail, port);
                }
            }
            for (name, port) in &ws.arrays {
                let retail: Vec<i64> = match *name {
                    "hate" => p.hate.iter().map(|&v| v as i64).collect(),
                    "war" => p.war.iter().map(|&v| (v != 0) as i64).collect(),
                    "cooldown" => p.cooldown.iter().map(|&v| v as i64).collect(),
                    "spell_ent" => p.spell_ent.iter().map(|&v| v as i64).collect(),
                    "levels" => p.levels.iter().map(|&v| v as i64).collect(),
                    "sel" => p.sel.iter().map(|&v| v as i64).collect(),
                    "ring" => p.ring.iter().map(|&v| v as i64).collect(),
                    "xp_bank" => p.xp_bank.iter().map(|&v| v as i64).collect(),
                    "xp_vol" => p.xp_vol.iter().map(|&v| v as i64).collect(),
                    "balloons" => p.balloons.iter().map(|&v| v as i64).collect(),
                    "guards" => p.guards.0.iter().map(|&v| v as i64).collect(),
                    _ => continue,
                };
                // Retail's length rules, the port padded with 0: an
                // owner the port holds NO register for must read as
                // all-empty, not as "nothing to compare".
                for (i, &a) in retail.iter().enumerate() {
                    let b = port.get(i).copied().unwrap_or(0);
                    if a != b {
                        self.wiz_hit((ws.wiz, name), t, i as u16, a, b);
                    }
                }
            }
        }
    }

    /// Diff the port's free list against the recording's, filtered the
    /// way the importer itself filters it — so any difference is the
    /// port's own allocator ORDER, never the importer's census.
    ///
    /// This is the widest ungraded lane in the harness: a port that
    /// pushes a freed slot at the wrong moment, or frees a different
    /// NUMBER of slots, reads clean in pair mode forever and only bites
    /// a free run, as balanced same-`(class, model)` missing/extra rows
    /// once the two allocators hand out different slots for one spawn.
    pub(crate) fn compare_free_mc1(
        &mut self,
        world: &World,
        st: &RetailMc1,
        human_slot: u16,
        t: u64,
    ) {
        let pool = st.ents.len();
        let want: Vec<u16> = st
            .free_stack
            .iter()
            .copied()
            .filter(|&s| (s as usize) < pool && s != human_slot && st.ents[s as usize].class64 == 0)
            .collect();
        let got = world.free_stack_mc1();
        self.free.1 += 1;
        if want != got {
            self.free.0 += 1;
            if self.free.2.is_empty() {
                // Depth from the TOP is what matters: the next spawn
                // pops the end, so depth 0 diverging is a slot handed
                // out wrong THIS tick.
                let depth = want
                    .iter()
                    .rev()
                    .zip(got.iter().rev())
                    .position(|(a, b)| a != b);
                self.free.2 = format!(
                    "t={t} len retail {} port {}, top retail {:?} port {:?}, first top-diff depth {}",
                    want.len(),
                    got.len(),
                    want.last(),
                    got.last(),
                    depth.map_or("none (prefix)".into(), |d| d.to_string()),
                );
            }
        }
    }

    /// THE BUCKET CHAINS, link by link (round 153): for every member
    /// of a port tick-top chain, retail's settled `+0` must name the
    /// port's next member (slot 0 at the tail). Rows land in the
    /// entity table under `chain0` keyed by the member's class/model;
    /// severed chains are skipped. The human's out-of-pool carpet is
    /// spliced out of the port's class-3 list, so a retail link that
    /// names the human slot is compared against the member AFTER him.
    pub(crate) fn compare_chains_mc1(
        &mut self,
        world: &World,
        st: &RetailMc1,
        human_slot: u16,
        t: u64,
    ) {
        for (_name, list, intact) in world.chains_shadow_mc1() {
            if !intact {
                self.chain_n.1 += list.len() as u64;
                continue;
            }
            for (k, &s) in list.iter().enumerate() {
                if s == human_slot || s == PLAYER_TARGET_U16 {
                    continue;
                }
                let Some(r) = st.ents.get(s as usize) else { continue };
                if r.class64 == 0 {
                    self.chain_n.1 += 1;
                    continue;
                }
                self.chain_n.0 += 1;
                let want = r.chain_next as i64;
                let got = list.get(k + 1).copied().unwrap_or(0) as i64;
                // Retail's chain threads THROUGH the human's record;
                // the port's list has no such member.
                if want == human_slot as i64 && got != want {
                    continue;
                }
                if want != got {
                    self.hit((r.class64, r.model65, "chain0"), t, s, want, got);
                }
            }
        }
    }

    /// The MC1 world globals (`spawn_count[20]`, `erupting`, `plume`) —
    /// keyed into the wizard-lane table under wiz 255 so the report
    /// needs no third block. Imported every pair, never compared
    /// before round 153.
    pub(crate) fn compare_globals_mc1(&mut self, world: &World, st: &RetailMc1, t: u64) {
        let (sc, erupting, plume) = world.globals_shadow_mc1();
        for (i, (&a, &b)) in st.spawn_count.iter().zip(sc.iter()).enumerate() {
            if a != b {
                self.wiz_hit((255, "spawn_count"), t, i as u16, a as i64, b as i64);
            }
        }
        if st.erupting != erupting {
            self.wiz_hit((255, "erupting"), t, 0, st.erupting as i64, erupting as i64);
        }
        if st.plume != plume {
            self.wiz_hit((255, "plume"), t, 0, st.plume as i64, plume as i64);
        }
    }

    /// The report block. `by_first` orders lanes by the tick they FIRST
    /// part rather than by family — the free run's question is "what
    /// broke first", the pair run's is "which family is worst".
    pub(crate) fn render(&self, by_first: bool) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let total: u64 = self.lanes.values().map(|l| l.rows).sum();
        let _ = writeln!(
            s,
            "  RAW SHADOW (every ungraded per-entity lane): {total} mismatches"
        );
        let mut keys: Vec<_> = self.lanes.iter().collect();
        if by_first {
            keys.sort_by_key(|(k, l)| (l.first_t, k.0, k.1, k.2));
        }
        for (k, lane) in keys {
            let _ = writeln!(
                s,
                "    ({:>3},{:>3}) {}: {} rows t={}..{} across {} slot(s)  e.g. {}",
                k.0,
                k.1,
                k.2,
                lane.rows,
                lane.first_t,
                lane.last_t,
                lane.slots.len(),
                lane.example
            );
        }
        // ⚠⚠ THE EXCLUSION CENSUS — see [`Shadow::hidden`]. Printed
        // right under the mismatch headline, because it is the
        // denominator that headline is missing: every slot named here
        // had ALL its ungraded lanes dropped, so a lane that is wrong
        // on a swarm body forever still reads as 0 rows above.
        if self.ent_n > 0 {
            let htotal: u64 = self.hidden.values().sum();
            if htotal == 0 {
                let _ = writeln!(
                    s,
                    "    tear gate: 0 slot-exclusions over {} boundaries — every live slot \
                     was shadowed",
                    self.ent_n
                );
            } else {
                let _ = writeln!(
                    s,
                    "    ⚠ TEAR GATE HID {htotal} slot-exclusion(s) over {} boundaries, \
                     {} distinct slot(s) — NOT shadowed on any ungraded lane \
                     (`torn_slots`: phase3e moved by an amount its species' \
                     CADENCE does not allow — see verify_mc2::slot_is_torn)",
                    self.ent_n,
                    self.hidden_slots.len()
                );
                let mut fam: Vec<_> = self.hidden.iter().collect();
                fam.sort_by_key(|(k, v)| (std::cmp::Reverse(**v), **k));
                for ((c, m), n) in fam.iter().take(8) {
                    let _ = writeln!(s, "      hidden ({c:>3},{m:>3}): {n}");
                }
                if fam.len() > 8 {
                    let _ = writeln!(
                        s,
                        "      … and {} more hidden (class, model)",
                        fam.len() - 8
                    );
                }
            }
        }
        if !self.skew.is_empty() {
            // A retail lane with no port twin. Loud once, because the
            // per-tick alternative is a flood and the silent
            // alternative is an under-count nobody notices.
            let _ = writeln!(
                s,
                "    ⚠ LANE TABLE SKEW — {} retail lane(s) absent from the port table \
                 (extend BOTH): {:?}",
                self.skew.len(),
                self.skew
            );
        }
        // THE OBJECTIVE BOARD. Printed on BOTH paths and BEFORE the
        // wizext block, because it is the lane a stage-gated
        // disposition rides and a single early latch is worth
        // thousands of entity rows (round 98 dig Q13's t=1200 wall).
        if !self.board_fed {
            let _ = writeln!(
                s,
                "  OBJECTIVE BOARD: NOT WATCHED on this path — `struct_0x3659C[local]` \
                 (level-end latch, row cursor, m32 pause, the 8 row states) is UNCHECKED here."
            );
        } else {
            let btotal: u64 = self.board.values().map(|l| l.rows).sum();
            let _ = writeln!(
                s,
                "  OBJECTIVE BOARD (struct_0x3659C[local]: IsLevelEnd/cursor/pause/8 row \
                 states): {btotal} mismatches over {} boundaries",
                self.board_n
            );
            let mut keys: Vec<_> = self.board.iter().collect();
            if by_first {
                keys.sort_by_key(|(k, l)| (l.first_t, k.0, k.1));
            }
            for (k, lane) in keys {
                let _ = writeln!(
                    s,
                    "    {}{}: {} rows t={}..{}  e.g. {}",
                    k.0,
                    if k.0 == "state" {
                        format!("[{}]", k.1)
                    } else {
                        String::new()
                    },
                    lane.rows,
                    lane.first_t,
                    lane.last_t,
                    lane.example
                );
            }
        }
        if !self.wiz_fed {
            // No WIZEXT arm ran on this path. Say so — "0 mismatches"
            // would read as "the player block is clean" when it means
            // "nobody looked", which is the exact instrument asymmetry
            // this module exists to end.
            let _ = writeln!(
                s,
                "  WIZEXT SHADOW: NOT WATCHED on this path — no per-player projection was \
                 fed, so the wizard block (charge, brain state, cooldowns, hate, the \
                 spell-book arrays) is UNCHECKED here."
            );
            let _ = writeln!(
                s,
                "    free stack: {} / {} boundaries mismatched{}",
                self.free.0,
                self.free.1,
                if self.free.2.is_empty() {
                    String::new()
                } else {
                    format!("  e.g. {}", self.free.2)
                }
            );
            let _ = writeln!(
                s,
                "    recycle stack: {} / {} boundaries mismatched{}",
                self.recycle.0,
                self.recycle.1,
                if self.recycle.2.is_empty() {
                    String::new()
                } else {
                    format!("  e.g. {}", self.recycle.2)
                }
            );
            return s;
        }
        let wiz_total: u64 = self.wiz_lanes.values().map(|l| l.rows).sum();
        let _ = writeln!(
            s,
            "  WIZEXT SHADOW (Type_160 wizard/brain lanes): {wiz_total} mismatches"
        );
        let _ = writeln!(
            s,
            "    compared (boundaries, skipped = eliminated or seated elsewhere): {}",
            self.wiz_n
                .iter()
                .map(|(w, (n, sk))| format!("wiz {w} ×{n} (skipped {sk})"))
                .collect::<Vec<_>>()
                .join(" · ")
        );
        let mut keys: Vec<_> = self.wiz_lanes.iter().collect();
        if by_first {
            keys.sort_by_key(|(k, l)| (l.first_t, k.0, k.1));
        }
        for (k, lane) in keys {
            // wiz 255 = the MC1 world globals (`compare_globals_mc1`).
            let who = if k.0 == 255 {
                "globals".to_string()
            } else {
                k.0.to_string()
            };
            let _ = writeln!(
                s,
                "    wiz {} {}: {} rows t={}..{} across {} idx  e.g. {}",
                who,
                k.1,
                lane.rows,
                lane.first_t,
                lane.last_t,
                lane.slots.len(),
                lane.example
            );
        }
        if self.chain_n.0 + self.chain_n.1 > 0 {
            let _ = writeln!(
                s,
                "    bucket chains: {} link(s) compared, {} skipped (severed / freed) — rows under `chain0`",
                self.chain_n.0,
                self.chain_n.1
            );
        }
        let _ = writeln!(
            s,
            "    free stack: {} / {} boundaries mismatched{}",
            self.free.0,
            self.free.1,
            if self.free.2.is_empty() {
                String::new()
            } else {
                format!("  e.g. {}", self.free.2)
            }
        );
        // ⚠ The RECYCLE verdict used to print ONLY on the `!wiz_fed`
        // path — i.e. never on MC2, the one column that HAS a recycle
        // stack (`compare_wiz_mc2` sets `wiz_fed`). The reporter was
        // dropping its own second allocator lane. Same shape as the
        // hole this whole module exists to close.
        if self.recycle.1 > 0 {
            let _ = writeln!(
                s,
                "    recycle stack: {} / {} boundaries mismatched{}",
                self.recycle.0,
                self.recycle.1,
                if self.recycle.2.is_empty() {
                    String::new()
                } else {
                    format!("  e.g. {}", self.recycle.2)
                }
            );
        }
        s
    }
}
