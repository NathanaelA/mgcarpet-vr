//! `verify-deltas`, MC2 arm (docs/RECORDING.md): import the raw
//! `D41A0_0` state at tick N onto a pristine-built MC2 world, tick
//! once with the human pinned to the recorded carpet pose, and diff
//! the port's obs projection against the recorded obs at N+1.
//!
//! MC2 capture specifics (measured on the mc2l0 corpus, 2026-07-30):
//! - The recorder has NO emit-time gate for MC2 (`tear_gate: false`),
//!   and the per-player `Turn` counter advances on EVERY adjacent
//!   pair — including torn ones — so Turn continuity alone cannot
//!   classify. Neither can global-LCG parity: MC2's draw count per
//!   tick is activity-dependent (0..16+, mode 1), and most FROZEN
//!   pairs still show exactly one draw.
//! - The working discriminator is the per-entity phase byte @0x3E
//!   (`byte_0x3E_62`, incremented once per handler run): across a
//!   true inter-tick pair the live-in-both entity population is
//!   step-1 dominant. A snapshot parked after Turn++ but BEFORE the
//!   entity pass yields an all-0 pair (positions frozen — measured
//!   moved-fraction 0.04) followed by an all-2 pair. ~30% of mc2l0
//!   pairs are torn this way. [`capture_clean_mc2`] encodes the law:
//!   d1 >= max(d0, d2) over deltas in {0, 1, 2} (larger deltas are
//!   animation wraps, not tear signal).
//!
//! Input: takes recorded before 2026-07-30 carry no input channel
//! (`channels.input: "none"`) — commands stay default and human casts
//! surface as capture families. Newer takes carry the MC2 raw
//! externals (held mouse buttons + the press LATCHES + cursor —
//! RECORDING.md), and the cast phase is not modelled by a delay knob
//! at all: the press latch says, per press, which side of retail's
//! own input poll the snapshot landed on. [`align_cmd_mc2`] is the
//! law and carries the derivation + the corpus measurement;
//! `--input-delay` is ignored on this arm (`MGC_CAST_RING=1` restores
//! the legacy ring for A/B).

use crate::Args;
use crate::verify::{FieldDiff, PairDiff, START_WARMUP, Stats};
/// The campaign REPLAY gate's capture witness — shared with the app's
/// own retail driver, which needs the identical derivation
/// ([`mgc_formats::mgcr::mc2_take_replayed`]).
pub(crate) use mgc_formats::mgcr::mc2_take_replayed;
use mgc_formats::mgcr::{EntObsMc2, ObsMc2, Recording, RetailEntMc2, RetailMc2, decode_retail_mc2};
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::conformance::{PinnedMc2, ThingTable};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use std::collections::BTreeMap;

/// THE MC2 POSE PAIR — the MC1 law's MC2 face
/// ([`crate::verify::pose_pair`], landed there as the default after
/// mc1l42 went 54,746 CSV rows -> 330). Retail's carpet moves MID-WALK
/// (`sub_5D530` at EF:59994, inside `AddPlayer03_00_5E010`, which the
/// walk reaches at the carpet's own pool slot), so NO single sample is
/// the right phase for the whole pass: a walker BELOW the carpet slot
/// reads the pose settled last frame (state@N) and one ABOVE the pose
/// this frame's mover just wrote (state@N+1). `World::tick_pose_pair`
/// already encodes exactly that for MC2 — `mc2_carpet_dispatch` takes
/// the `post` sample and swaps `*player` in place at the carpet slot
/// (engine/world.rs, the `else if let Some(p) = post` arm) — but this
/// arm never called it and kept the retired single-sample walk, so the
/// `pose-phase` tag has been absorbing the difference: 24,467 of
/// mc2l22's 130,624 CSV rows, 20,056 of them the (9,1) possession
/// bolt's own birth (its manifestation token sits at pool slot 40,
/// far below the carpet at 424, so retail launches it from the
/// PRE-move pose while this arm pinned N+1).
///
/// DEFAULT ON, exactly as the MC1 twin ended up: the staging opt-in
/// (`MGC_MC2_POSE_PAIR=1`) has served its purpose — every frozen MC2
/// fixture signature and the certified mc2l0 / mc2l3 takes were
/// re-graded under the two-phase walk. `MGC_NO_MC2_POSE_PAIR=1`
/// restores the retired single-sample walk for A/B.
pub(crate) fn mc2_pose_pair() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_POSE_PAIR").is_none())
}

/// ⚠ **OPT-IN, AND DELIBERATELY NOT THE DEFAULT**
/// (`MGC_MC2_JITTER_GROUND=1`). The DIAGNOSIS behind this is solid: the
/// (10,71) fissure's tick adds `life & 1 ? +1 : -1` to
/// `mapHeightmap_11B4E0` across its whole disc EVERY tick
/// (EF:29529-37 / :29560-69), so the recorded terrain endpoints
/// alternate by one height unit under the carpet — and one height unit
/// is 32 engine units (`32*p1`, Terrain.cpp:113-172), exactly the size
/// of the `pose.z` residue. Retail's mover resolves its floor against
/// whichever phase stood at its OWN walk slot, an image neither
/// recorded endpoint holds.
///
/// What is NOT known is WHICH phase. Retail demonstrably reads the
/// RAISED image on both parities in the t=24874 and t=26120 windows and
/// the LOWERED one in t=24746-24764 — **no single-endpoint rule and no
/// fixed slot order expresses that**, so the "cells that moved by
/// exactly +-1 take the higher endpoint" rule below is a HEURISTIC
/// fitted to the corpus, not retail's law. It fixes 163 rows and
/// INTRODUCES 83.
///
/// ⭐⭐⭐ A GRADING ORACLE TUNED TO ITS OWN SCORE CAN HIDE A PORT BUG,
/// so the default stays on the settled `measured@N+1` image and the
/// reported pose number stays honest. Turn this on to work the
/// mechanism, not to book the rows.
pub(crate) fn mc2_jitter_ground() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_MC2_JITTER_GROUND").is_some())
}

/// ⭐⭐⭐ THE CARPET'S GROUND PROBE RUNS AT ITS OWN WALK SLOT, SO A
/// HIGHER-SLOT TERRAFORM OF THE SAME TICK IS INVISIBLE TO IT.
/// `sub_5D530` calls `getTerrainAlt_10C40(&predictedAxis)`
/// (`NETHERW.EXE` file 0x81e57 `push 0x1b398` / 0x81e5f `call
/// 0x35440` = VA 0x10C40 -> `sub_B5C60_getTerrainAlt2`) and folds the
/// result into BOTH the climb-band authority (`altDiff`,
/// 0x81e6a..0x81e82) and the post-gate floor clamp. This arm installed
/// measured terrain@N+1 and handed the mover THAT, so every same-tick
/// terraform reached the carpet a tick early — while the MC1 arm has
/// always phased its ground per cell by the PORT's own mid-walk
/// witness ([`crate::verify::midwalk_ground`], verify.rs, ungated).
/// The law here is simply that same oracle.
///
/// WITNESS (mc2l24, 2026-09-10): the (10,42) painter at slot 701 —
/// above the human's slot 116 — lowers the height plane 2-3 units per
/// tick under the carpet at t=26209+ and t=26354+. One height byte is
/// 32 engine units, so terrain@N vs @N+1 is tens of units of
/// authority: at t=26209 `h(81,136)` is 49 at the carpet's slot and 47
/// after the painter => `getTerrainAlt` 1550 vs 1486 => `altDiff` -232
/// vs -168 => `eff_pitch` 94 (retail) vs 68 (what this arm reported).
/// terrain@N reproduces retail to the unit on every row of both
/// clusters. The converse also exists — mc2l0 t=5306 and mc2l3 t=243
/// (the castle BUILD00 painter) terraform BELOW the carpet's slot and
/// retail sees those writes the same tick — which is why the phase is
/// PER CELL and a flat "always @N" costs rows there (measured: mc2l0
/// 22380 -> 22374, mc2l1 18405 -> 18400, mc2l3 22355 -> 22337
/// bit-exact pose pairs).
///
/// `MGC_NO_MC2_CARPET_GROUND_AT_N=1` restores the terrain@N+1 probe.
pub(crate) fn no_mc2_carpet_ground_at_n() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_NO_MC2_CARPET_GROUND_AT_N").is_some())
}

/// W2-6 A/B (`MGC_MC2_MIDWALK_GROUND=1`): use the MC1 arm's
/// port-witnessed per-cell phase oracle for the MC2 pose lane's ground
/// image instead of the raised-phase reconstruction.
pub(crate) fn mc2_midwalk_ground() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("MGC_MC2_MIDWALK_GROUND").is_some())
}

pub(crate) fn run(path: &std::path::Path, args: &Args) -> Result<bool, String> {
    let pin_n1 = match args.pin_pose.as_str() {
        "n" => false,
        "n1" => true,
        other => return Err(format!("--pin-pose {other:?}: want n or n1")),
    };
    let mut rec = Recording::open(path)?;
    let level = rec.header.level.ok_or("recording has no level number")?;
    println!(
        "== verify-deltas {} (game mc2, level {level}, pin-pose {})",
        path.display(),
        args.pin_pose
    );
    let replayed = mc2_take_replayed(path)?;
    let (mut world, pristine, things) = build_world_mc2(&args.baked, level, replayed)?;

    let mut csv: Option<std::io::BufWriter<std::fs::File>> = match &args.csv {
        Some(p) => {
            let f = std::fs::File::create(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut w = std::io::BufWriter::new(f);
            use std::io::Write as _;
            writeln!(
                w,
                "t\tkind\tslot\tclass\tmodel\tfield\twant\tgot\tx\ty\tz\trule"
            )
            .map_err(|e| e.to_string())?;
            Some(w)
        }
        None => None,
    };
    let roster = crate::verify::load_roster(args)?;
    let take = crate::verify::take_stem(path);

    let mut prev: Option<(u64, RetailMc2, PlayerCommand)> = None;
    let mut prev_cmd = PlayerCommand::default();
    // A/B escape hatch for the fire-edge revival below (ledger
    // §"THE PORT'S CAST EDGE WAS DEAD IN THE HARNESS"): with this set
    // the predecessor never advances, i.e. the pre-fix behaviour.
    // Ring mode only — the aligned arm carries no such lane.
    let freeze_prev_cmd = std::env::var_os("MGC_NO_FIRE_EDGE").is_some();
    // The latch-aligned cast phase (see [`align_cmd_mc2`]) is the law;
    // `MGC_CAST_RING=1` restores the legacy `--input-delay` ring for
    // A/B. Aligned mode ignores `--input-delay` (it has no free knob:
    // the recorder's own latch says which side of retail's poll each
    // press landed on).
    let ring_mode = std::env::var_os("MGC_CAST_RING").is_some();
    let mut cmd_ring: std::collections::VecDeque<PlayerCommand> =
        std::iter::repeat_n(PlayerCommand::default(), args.input_delay as usize + 1).collect();
    let mut prev_latch = (false, false);
    // The cursor-AT-PRESS lane. `press_edge_mc2` documents the A/B and
    // its measurement; the detector below is always fed so the run can
    // report the lane's traffic even with the fold off.
    let press_edge_mode = std::env::var_os("MGC_PRESS_EDGE").is_some();
    // Consumed-byte fire is the default (see the override below);
    // `MGC_PRESS_LATCH=1` restores the latch-only fold for A/B.
    let press_latch_mode = std::env::var_os("MGC_PRESS_LATCH").is_some();
    let mut prev_press: Option<(i16, i16)> = None;
    let mut press_moves = 0u64;
    // ⭐⭐⭐ THE RESPAWN LANE IS `mgc_formats::recover::Mc2RespawnWitness`,
    // NOT A SECOND COPY OF IT. This loop used to hand-roll the dating
    // rule (`space && (prev_space || (mouse != prev_mouse && mouse ==
    // press))`) — the PRE-ROUND-146 form, the one
    // `MGC_NO_MC2_RESPAWN_CENTRE_POINT=1` restores — while the free
    // runner (`replay.rs`) had long since moved to the shared witness,
    // whose own doc comment names *mc2l22-new record 27599* as the
    // press the cursor-equality rule misses. The pair runner therefore
    // never respawned the human on that boundary at all: retail's
    // `[REVIVED]` at t=27598→27599 rewrote `charge`, `invuln`,
    // `regen_stall`, `knock_dir`, `life_regen` and all 26 `spell_ent`
    // book slots, and every one of them read as a WIZEXT pair row
    // (31 rows on one boundary) because the port's tick simply did not
    // take the transition. See round 148, dig w148f.
    // `MGC_NO_MC2_PAIR_RESPAWN_WITNESS=1` restores the hand-rolled copy
    // for A/B.
    let pair_respawn_legacy = std::env::var_os("MGC_NO_MC2_PAIR_RESPAWN_WITNESS").is_some();
    let mut respawn_witness = mgc_formats::recover::Mc2RespawnWitness::default();
    let mut prev_space = false;
    let mut prev_mouse: Option<(i16, i16)> = None;
    // The cycle-ring cast lane (`ring_cast_mc2`): unreachable on today's
    // corpus, LOUD if a take ever trips it.
    let ring_bit_off = std::env::var_os("MGC_NO_HAND_BIT").is_some();
    let mut ring_casts = 0u64;
    let mut stats = Stats::default();
    let mut pose_chan = crate::pose_lane::PoseLane::default();
    // THE RAW SHADOW (`MGC_RAW_SHADOW=1`) — the MC2 arm. `EntObsMc2`
    // carries 20 of the record's 91 raw lanes, so a handler that reads
    // one of the other 71 correctly and WRITES it wrong reports CLEAN
    // in pair mode forever. That is not hypothetical on this game: the
    // mc2l0 horizon head was a `phase3e` reading 1 against retail's
    // 149 — invisible to the graded diff, and the actual cause of the
    // two graded rows it did show. Off by default; it grades nothing,
    // it only reports, and it must not move the UNEXPLAINED headline.
    let mut shadow = crate::shadow::Shadow::from_env()?;
    // `MGC_STAGE_TRACE` — the OBJECTIVE BOARD microscope, wired on
    // the PAIR path too. In the free run it shows where the port's
    // own board history parts; here the importer has just restored
    // retail's board, so it shows which handler failed to move it
    // during the one tick that ran. (Print-only, default-OFF.)
    let mut stagetrace = crate::alloc_trace::StageTrace::from_env();
    // The PER-ENTITY tear census (see the loop body): pairs touched,
    // total slot-exclusions, distinct slots, and the (class, model)
    // breakdown — a family concentration is the tell that an
    // exclusion is hiding a law rather than a capture artifact.
    let (mut torn_pairs, mut torn_rows) = (0u64, 0u64);
    let mut torn_distinct: std::collections::BTreeSet<u16> = Default::default();
    let mut torn_fam: std::collections::BTreeMap<(u8, u8), u64> = Default::default();
    let mut printed_import = false;
    let mut boundary_seeded = false;
    // Measured-terrain accumulator — the MC1 twin's pending-block
    // pattern (verify.rs): a pair (pt → t) runs on terrain AT pt.
    let mut timg = (!args.no_terrain)
        .then(|| {
            rec.header
                .channels
                .terrain
                .as_ref()
                .map(mgc_formats::mgcr::TerrainImage::new)
        })
        .flatten();
    // `--start <t>`: skip to t minus a warm-up window WITHOUT decoding
    // the records passed over (their terrain deltas still fold). The
    // window keeps every one-record carry the pair lane reads — the
    // pairing chain, the input latch/press/mouse predecessors, the
    // legacy cast ring — identical to a whole-stream walk; only the
    // whole-stream input counters now start here, and say so.
    crate::slice_banner(&rec);
    let skip_from = args.start.map(|s| s.saturating_sub(START_WARMUP));
    if let Some(s) = skip_from {
        crate::late_tick_hint(&rec, path, s);
        rec.skip_to(s, timg.as_mut())?;
    }
    let mut pending_terrain: Option<mgc_formats::mgcr::TerrainBlock> = None;
    // Terrain@N held across the pose lane's in-place advance to N+1 —
    // see [`crate::verify::PlanesAtN`]. Both re-execs below (pose-alt,
    // `--dump`) are still executing the PAIR and must see terrain@N.
    let mut planes_n = crate::verify::PlanesAtN::default();
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        if let Some(img) = timg.as_mut() {
            if let Some(block) = pending_terrain.take() {
                img.apply(&block)
                    .map_err(|e| format!("t={}: terrain: {e}", tick.t))?;
            }
            pending_terrain = tick.terrain.clone();
        }
        let Some(state) = &tick.state else {
            prev = None;
            continue;
        };
        let st = decode_retail_mc2(state)?;
        let obs: ObsMc2 = match &tick.obs {
            Some(v) => serde_json::from_str(v.get()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        let (held, latch) = raw_input_mc2(tick.input.as_ref());
        let mut aligned = align_cmd_mc2(held, latch, prev_latch);
        // THE CONSUMED-BYTE FIRE LAW (recover_pair_mc2's measurement,
        // ledger §THE REPLAY VERIFIER): fire rides the consumed
        // move/fire byte on the pair's END record — 560/560 retail
        // arms carry the bit, and the press-latch fold's extra edges
        // are HUD clicks the byte correctly omits (mc2l3 t=105: the
        // equip click at press_pos (70,312) — the spell panel — folded
        // into a phantom fireball; retail routed it to the pane select
        // and the hand flips at t=109). The latch law's phase claim
        // stays true — the byte simply also carries retail's own
        // view-vs-panel routing. `MGC_PRESS_LATCH=1` restores the
        // latch-only fold for A/B.
        if !press_latch_mode && let Some(p) = st.players.get(st.local_player as usize) {
            let (fl, fr) = mgc_formats::recover::mc1_fire(p.move_bits);
            aligned.fire_left = fl;
            aligned.fire_right = fr;
        }
        // THE THIRD CAST ARM rides the SAME consumed byte
        // (`recover::mc2_ring_cast`) — see `ring_cast_mc2` below.
        if let Some(p) = st.players.get(st.local_player as usize) {
            aligned.mc2_ring_cast = ring_cast_mc2(p);
        }
        let press = press_pos_mc2(tick.input.as_ref());
        // The respawn key rides the pair's END record like the aligned
        // cast bits — see [`respawn_key_mc2`] for the two witnesses
        // that date the press against retail's poll.
        let witness = respawn_witness.observe(tick.input.as_ref());
        let space = respawn_key_mc2(tick.input.as_ref());
        let mouse = mgc_formats::recover::mouse_pos(tick.input.as_ref());
        let legacy = space
            && (prev_space || (mouse.is_some() && mouse != prev_mouse && mouse == press));
        prev_space = space;
        prev_mouse = mouse.or(prev_mouse);
        aligned.respawn = if pair_respawn_legacy { legacy } else { witness };
        let moved = matches!((prev_press, press), (Some(a), Some(b)) if a != b);
        press_moves += u64::from(moved);
        if press_edge_mode {
            aligned = press_edge_mc2(aligned, held, latch, moved);
        }
        prev_press = press.or(prev_press);
        if !ring_bit_off
            && let Some(p) = st.players.get(st.local_player as usize)
            && let Some(spell) = ring_cast_mc2(p)
        {
            ring_casts += 1;
            if std::env::var_os("MGC_RING_CAST_TRACE").is_some() {
                eprintln!("  t={}: CYCLE-RING CAST (0x40) spell {spell}", tick.t);
            }
        }
        if ring_bit_off {
            aligned.mc2_ring_cast = None;
        }
        prev_latch = latch;
        let sample = if ring_mode {
            sample_cmd_mc2(tick.input.as_ref())
        } else {
            PlayerCommand::default()
        };
        if ring_mode && !boundary_seeded && tick.input.is_some() {
            boundary_seeded = true;
            // A button already held on the recording's FIRST frame has
            // no press edge inside the capture (retail latched it
            // before t=0), but the ring's default pre-fill reads
            // "released" and manufactures one — the t≈3 (9,17)-vs-
            // smoke misfire was the right button held across the
            // level boundary. Extend the first frame's held state
            // backward instead. (Aligned mode needs no seed: its
            // predecessor is the previous RECORD's own level.)
            for c in cmd_ring.iter_mut() {
                *c = sample;
            }
            prev_cmd = sample;
        }
        let cmd = if ring_mode {
            cmd_ring.push_back(sample);
            cmd_ring.pop_front().unwrap_or_default()
        } else {
            aligned
        };
        if let Some((pt, pst, pcmd)) = prev.take() {
            // THE PAIR'S COMMAND AND ITS PREDECESSOR. Aligned mode reads
            // the pair's END record (this iteration's `aligned`) — that
            // is the input frame pt+1 polled, and the pair IS frame
            // pt+1's transition ([`align_cmd_mc2`]). The legacy ring
            // instead hands the pair the delayed sample stored with its
            // START record.
            let (pair_cmd, pair_prev) = if ring_mode {
                (pcmd, prev_cmd)
            } else {
                (cmd, pcmd)
            };
            // The recovered PANE SELECT (recover_pair_mc2's rebind
            // law — the free runner's lane, folded here too): a
            // recorded hand change across the pair replays as the
            // equip. Without it every equip pair left the port's
            // hand one spell behind (the 12-row player0.hand_left
            // family, mc2l3 t=108: the (70,312) panel click's flip).
            let (pair_cmd, pair_full_stop) = {
                let mut c = pair_cmd;
                let rec =
                    mgc_formats::recover::recover_pair_mc2(&pst, &st, false, tick.input.as_ref());
                c.mc2_select = rec.mc2_select;
                // …and the pane's SHIFT+click ring bind, which shares
                // the handler's sound and the same "reconstruct input
                // IDENTICALLY in every fold" rule (CONFORMANCE.md).
                c.spell_ring = rec.spell_ring;
                // A recorded retail cheat mutates the world, so the
                // pair it fires on cannot conform without it
                // (`engine::world::cheats`).
                c.cheat = rec.cheat;
                // …and so does a demolish. MC2 has no move-byte trace
                // for it (`PlayerAction` 0x2A), so `recover_pair_mc2`
                // reads the own castle at the END record with
                // `life == -1` in the destroy intake; MC1's verify arm
                // has always forwarded its own witness (verify.rs) and
                // this one silently did not, leaving MC2 pair mode
                // structurally unable to demolish.
                c.demolish = rec.demolish;
                // …and the Shift+K witness (134-7) — the same fold
                // as the suite's (fixtures.rs): the pair recovery
                // grew a `suicide:` row and this arm has to forward
                // it, or `verify-deltas` and the suite grade a live
                // human where retail's own key killed it.
                c.suicide = rec.suicide;
                // The FULL STOP travels with them: `PlayerEvents_51BB0`
                // case 0x27 is an input-pass write the pair importer
                // cannot see from the state at N
                // (`World::mc2_full_stop_import`).
                (c, rec.mc2_park)
            };
            if args.start.is_some_and(|s| pt < s) {
                // Before the triage window — keep the pairing chain
                // and the input ring warm, execute nothing.
            } else if tick.t == pt + 1 {
                let announce = args.start.is_some();
                if announce {
                    eprintln!("pair {pt}");
                }
                if std::env::var_os("MGC_CAST_TRACE").is_some()
                    && ((pair_cmd.fire_right && !pair_prev.fire_right)
                        || (pair_cmd.fire_left && !pair_prev.fire_left))
                {
                    eprintln!(
                        "CASTEDGE pair {pt} L{} R{}",
                        u8::from(pair_cmd.fire_left && !pair_prev.fire_left),
                        u8::from(pair_cmd.fire_right && !pair_prev.fire_right)
                    );
                }
                stats.pairs += 1;
                // DIG5 PROBE: stamp the pair tick for the sim-side
                // probes (MGC_CARPET_PROBE / MGC_WRITE_TRACE / …).
                // `verify.rs:318` does this on the MC1 column; the MC2
                // column never did, so every DEBUG_TICK-gated
                // instrument silently no-ops here.
                mgc_sim::DEBUG_TICK.store(pt, std::sync::atomic::Ordering::Relaxed);
                if !capture_clean_mc2(&pst, &st) {
                    stats.torn += 1;
                } else {
                    // W2-6: arm the pose channel's MID-WALK ground
                    // snapshot — the port's height plane as its walk
                    // crosses the carpet's own slot, which is
                    // `sub_5D530`'s `getTerrainAlt` probe phase.
                    world.arm_midtick_ground_snapshot();
                    let (pd, port, report) = exec_pair_mc2(
                        &mut world,
                        &pristine,
                        crate::verify::measured_planes(&timg),
                        &things,
                        &pst,
                        &st,
                        &obs,
                        pair_cmd,
                        pair_prev,
                        if mc2_pose_pair() {
                            crate::verify::PairPose::Pair
                        } else if pin_n1 {
                            crate::verify::PairPose::PinN1
                        } else {
                            crate::verify::PairPose::PinN
                        },
                        pair_full_stop,
                    )
                    .map_err(|e| format!("t={pt}: {e}"))?;
                    let human_slot = report.human_slot;
                    // ⚠ THE PER-ENTITY TEAR IS AN EXCLUSION, AND AN
                    // UNREPORTED EXCLUSION IS A BLIND SPOT. Every slot
                    // in here had ALL its fields dropped from the
                    // graded diff — so a lane can be wrong on a torn
                    // slot forever and the report will never say so.
                    // The whole-pair `0 TORN` headline is a DIFFERENT
                    // number (`capture_clean_mc2`); this one is
                    // per-entity and used to be invisible. (mc2l0
                    // t=1055: retail recycles a castle piece, its
                    // phase byte goes 255 → 230, and the slot's real
                    // x/y divergence was silently skipped — that
                    // absence read as evidence of correctness.)
                    let torn = torn_slots(&pst, &st);
                    if !torn.is_empty() {
                        torn_pairs += 1;
                        torn_rows += torn.len() as u64;
                        for s in &torn {
                            torn_distinct.insert(*s);
                            let e = &st.ents[*s as usize];
                            *torn_fam.entry((e.class3f, e.model40)).or_insert(0u64) += 1;
                        }
                    }
                    // Feed the raw shadow HERE — the world is exactly
                    // the post-tick state the graded diff just judged,
                    // and the pose lane below re-installs terrain.
                    //
                    // ⭐⭐⭐ **A SHADOW ROW NAMES THE BOUNDARY IT
                    // COMPARED, NOT THE PAIR THAT REACHED IT** (round
                    // 149). Every `compare_*` below is `port-after-the
                    // -tick` vs `st`, and `st` is retail's record at
                    // **`pt + 1`** — so stamping these rows `pt` said
                    // "t=59885" about a boundary the FREE-RUN shadow
                    // (`replay.rs`, which certified the corpus) and
                    // `dump-state --port` both call 59886. Two
                    // instruments, one boundary, two numbers: round
                    // 148 lost a dig to exactly that on mc2l24-crazy's
                    // (5,25) `target96`/`f2e`.
                    //
                    // The PAIR-scoped output (`PairDiff` rows,
                    // `--dump`, the fixture manifests, `DEBUG_TICK`
                    // and every probe hung off it) keeps `pt`: a pair
                    // diff is a statement about the pair and the whole
                    // fixture corpus is keyed on its start tick. Only
                    // the BOUNDARY-scoped shadow moves, and it is
                    // stdout-only and ungraded — nothing is keyed on
                    // it. `bt` is `tick.t` (the pair branch is
                    // `tick.t == pt + 1` by construction — see the
                    // `anchor` test above).
                    let bt = pt + 1;
                    if let Some(sh) = shadow.as_mut() {
                        sh.compare_ents_mc2(&world, &st, human_slot, &torn, bt);
                        sh.compare_map_heads_mc2(&world, &st, human_slot, &torn, bt);
                        sh.compare_wiz_mc2(&world, &st, bt);
                        // THE OBJECTIVE BOARD — recorded since the
                        // capture was written, ungraded until round
                        // 98. Here the importer restored it from
                        // retail@t before the tick, so a row is a
                        // one-tick WRITE bug.
                        sh.compare_board_mc2(&world, &st, bt);
                        // A fallback pair started from a SCANNED free
                        // list, not retail's, so it has nothing to say
                        // about the allocator.
                        if report.stack_fallback.is_none() {
                            sh.compare_free_mc2(&world, &st, human_slot, bt);
                        }
                    }
                    if let Some(sg) = stagetrace.as_mut() {
                        sg.emit(&world, &st, pt);
                    }
                    // ARM terrain@N before the pose lane advances the
                    // shared image: a re-exec runs below iff the pair
                    // is dirty (pose-alt, and `--dump-first`) or this
                    // tick was named by `--dump`, and it only needs
                    // the snapshot when the tick actually carries a
                    // terraform block.
                    if pending_terrain.is_some() && (!pd.clean() || args.dump == Some(pt)) {
                        planes_n.arm(&timg);
                    } else {
                        planes_n.disarm();
                    }
                    // The POSE CHANNEL (crate::pose_lane): shadow-step
                    // the faithful mover over the human's own motion
                    // column. The world takes measured terrain@N+1
                    // (the pending-block re-apply at the loop top is
                    // idempotent — deltas carry absolute values), but
                    // the MOVER's own ground closure is handed
                    // terrain@N: the carpet probes `getTerrainAlt` at
                    // its own walk slot, so a higher-slot terraform of
                    // the same tick is invisible to it (see
                    // [`no_mc2_carpet_ground_at_n`]). ⚠ THE OLD
                    // COMMENT HERE CLAIMED "the same phase argument as
                    // the MC1 arm" — the MC1 arm has ALWAYS used the
                    // port-witnessed per-cell oracle
                    // ([`crate::verify::midwalk_ground`], verify.rs,
                    // ungated); only this arm was hard-wired to N+1.
                    if !args.no_pose_lane {
                        // W2-6: the ORACLE's two port-side witnesses —
                        // the height plane as the walk crossed the
                        // carpet's slot, and the port's settled
                        // post-tick plane — plus measured@N, all
                        // cloned BEFORE the measured@N+1 re-install
                        // below. Same three inputs the MC1 arm feeds
                        // `midwalk_ground`.
                        // The three witnesses the per-cell phase
                        // oracle needs (see
                        // [`no_mc2_carpet_ground_at_n`]): the port's
                        // plane AS ITS WALK CROSSED THE CARPET, its
                        // settled post-tick plane, and measured@N.
                        let law = !no_mc2_carpet_ground_at_n();
                        let on = mc2_jitter_ground() || law;
                        let snap = world.take_midtick_ground_snapshot().filter(|_| on);
                        let h_post: Option<Vec<u8>> =
                            snap.is_some().then(|| world.planes().height.clone());
                        let h_start: Option<Vec<u8>> = on
                            .then(|| {
                                crate::verify::measured_planes(&timg).map(|(h, _, _, _)| h.to_vec())
                            })
                            .flatten();
                        if let (Some(img), Some(block)) = (timg.as_mut(), pending_terrain.as_ref())
                        {
                            img.apply(block)
                                .map_err(|e| format!("t={pt}: pose terrain: {e}"))?;
                        }
                        if let Some((h, ty, ceil, an)) = crate::verify::measured_planes(&timg) {
                            world
                                .install_measured_terrain(h, ty, ceil, an)
                                .map_err(|e| format!("t={pt}: pose terrain: {e}"))?;
                        }
                        // ⭐ THE (10,71) FISSURE VIBRATES THE FLOOR BY
                        // ±1 HEIGHT UNIT EVERY TICK (`sub_3A2D0`,
                        // EF:29529-37 / 29560-69: `sign = life & 1`,
                        // added to `mapHeightmap_11B4E0` over a
                        // radius-`v6` disc, `life--` at the tail), so
                        // the recorded endpoints ALTERNATE under the
                        // carpet while retail's mover reads whichever
                        // phase stood at its own walk slot. One height
                        // unit is 32 engine units (`32*p1` in
                        // `sub_B5C60_getTerrainAlt2`) and the carpet
                        // floor-clamps to `locAlt + 256`, which is the
                        // whole ±32 pose.z family on mc2l6-rsg. Same
                        // per-cell phase oracle the MC1 arm has used
                        // since the t=567/t=1210 families: a cell
                        // keeps measured@N exactly when the port
                        // DEMONSTRABLY wrote it after the carpet
                        // (untouched at the snapshot, changed by tick
                        // end); every other cell takes measured@N+1.
                        //
                        // ⚠ THE PORT-WITNESSED ORACLE IS NOT ENOUGH
                        // HERE, and that is itself a measurement: the
                        // port's fissure token sits ABOVE the carpet
                        // on every pair, so the oracle picks
                        // measured@N uniformly and merely SWAPS which
                        // parity fails (26 → 24 rows on t=24874+130,
                        // 42 → 41 on t=26120+130). Retail's carpet
                        // demonstrably reads the RAISED phase on BOTH
                        // parities across those windows — @N when the
                        // tick lowers, @N+1 when it raises — which no
                        // single-endpoint rule can express. So the
                        // DEFAULT here reconstructs that phase
                        // directly: a cell the tick moved by exactly
                        // ±1 (the fissure's signature; a real
                        // terraform moves more) takes the HIGHER
                        // endpoint. `MGC_MC2_MIDWALK_GROUND=1` selects
                        // the MC1 oracle instead, for A/B.
                        let ground_mid: Option<Vec<u8>> = if law || mc2_midwalk_ground() {
                            match (snap, h_start, h_post, crate::verify::measured_planes(&timg)) {
                                (Some(snap), Some(h0), Some(post), Some((h1, _, _, _))) => {
                                    Some(crate::verify::midwalk_ground(snap, &h0, &post, h1))
                                }
                                (snap, ..) => snap,
                            }
                        } else {
                            match (h_start, crate::verify::measured_planes(&timg)) {
                                (Some(h0), Some((h1, _, _, _))) if h0.len() == h1.len() => Some(
                                    h0.iter()
                                        .zip(h1)
                                        .map(
                                            |(&a, &b)| {
                                                if a.abs_diff(b) == 1 { a.max(b) } else { b }
                                            },
                                        )
                                        .collect(),
                                ),
                                _ => None,
                            }
                        };
                        pose_chan
                            .run_pair_mc2(
                                &world,
                                ground_mid.as_deref(),
                                &pst,
                                &st,
                                human_slot,
                                pt,
                                // The full-stop key (BACKSPACE = 14) off
                                // the END record's raw input channel.
                                mgc_formats::recover::key_held(tick.input.as_ref(), 14),
                                csv.as_mut().map(|w| w as &mut dyn std::io::Write),
                            )
                            .map_err(|e| format!("t={pt}: pose csv: {e}"))?;
                    }
                    if announce && let Some((got, want)) = report.stack_fallback {
                        eprintln!("  free-stack fallback: live {got} != scan {want}");
                    }
                    // The full-pool allocator arm: `NewEvent_4A050`
                    // sacrifices a ranked recycle victim rather than
                    // dropping the spawn. Both counters together say
                    // what a full pool actually did on this pair.
                    let (seized, dropped) =
                        (world.take_recycle_seized(), world.take_pool_exhausted());
                    if seized != 0 || dropped != 0 {
                        eprintln!(
                            "  pair {pt}: {seized} recycle victim(s), {dropped} spawn(s) dropped"
                        );
                    }
                    if !printed_import {
                        printed_import = true;
                        println!(
                            "   import: {} active entities, human slot {human_slot}, terrain {}",
                            obs.n_active,
                            if crate::verify::measured_planes(&timg).is_some() {
                                "MEASURED"
                            } else {
                                "pristine"
                            }
                        );
                    }
                    stats.absorb_rng(pst.rand, obs.rng, port.rng);
                    let mut tags = (roster.is_some() || !args.no_pose_alt).then(|| {
                        let rmap: BTreeMap<u16, &EntObsMc2> =
                            obs.entities.iter().map(|e| (e.slot, e)).collect();
                        let pmap: BTreeMap<u16, &EntObsMc2> =
                            port.entities.iter().map(|e| (e.slot, e)).collect();
                        let ctx = |slot: u16| {
                            rmap.get(&slot)
                                .or_else(|| pmap.get(&slot))
                                .map(|e| (e.class, e.model, e.x, e.y))
                        };
                        let mut tg =
                            crate::verify::classify_pair(roster.as_ref(), &take, pt, &pd, &ctx);
                        // SLOT-DESYNC pass (computed rule, roster.rs) —
                        // the MC2 face of the wave slot-order desync
                        // (open-leads 0b). BEFORE pose-phase; see
                        // RuleTags::slot_desync.
                        if !args.no_slot_desync {
                            let pos = |slot: u16| ctx(slot).map(|(_, _, x, y)| (x, y));
                            tg.slot_desync(&pd.missing, &pd.extra, &pos);
                        }
                        tg
                    });
                    // Pose-phase pass — see verify.rs (the MC1 twin).
                    if !args.no_pose_alt
                        && !pd.clean()
                        && let Some(tg) = tags.as_mut()
                    {
                        let (alt, _, _) = exec_pair_mc2(
                            &mut world,
                            &pristine,
                            planes_n.get(&timg),
                            &things,
                            &pst,
                            &st,
                            &obs,
                            pair_cmd,
                            pair_prev,
                            // The phase ORACLE stays SINGLE-pinned even
                            // under the pair — the tag's meaning is
                            // defined against the opposite-endpoint
                            // probe (`crate::verify`'s MC1 twin).
                            if pin_n1 {
                                crate::verify::PairPose::PinN
                            } else {
                                crate::verify::PairPose::PinN1
                            },
                            pair_full_stop,
                        )
                        .map_err(|e| format!("t={pt}: pose-alt: {e}"))?;
                        crate::verify::pose_reclassify(tg, &pd, &alt);
                    }
                    let tags = tags;
                    if let Some(w) = csv.as_mut() {
                        emit_csv_mc2(w, pt, &pd, &obs, &port, roster.as_ref(), tags.as_ref())
                            .map_err(|e| e.to_string())?;
                    }
                    let dump = args.dump == Some(pt)
                        || (args.dump_first && !pd.clean() && stats.first_diff.is_none());
                    stats.absorb(pt, pd, tags.as_ref(), roster.as_ref(), args);
                    if dump {
                        let (pd, port, _) = exec_pair_mc2(
                            &mut world,
                            &pristine,
                            planes_n.get(&timg),
                            &things,
                            &pst,
                            &st,
                            &obs,
                            pair_cmd,
                            pair_prev,
                            if mc2_pose_pair() {
                                crate::verify::PairPose::Pair
                            } else if pin_n1 {
                                crate::verify::PairPose::PinN1
                            } else {
                                crate::verify::PairPose::PinN
                            },
                            pair_full_stop,
                        )
                        .map_err(|e| format!("t={pt}: {e}"))?;
                        print!("{}", pd.render(pt, usize::MAX));
                        if args.dump_port {
                            for e in &port.entities {
                                println!(
                                    "    port slot {}: cm=({},{}) life={}/{} \
                                     pos=({:.2},{:.2},{}) mana={} action={}",
                                    e.slot,
                                    e.class,
                                    e.model,
                                    e.life,
                                    e.max_life,
                                    e.x,
                                    e.y,
                                    e.z,
                                    e.mana,
                                    e.action
                                );
                            }
                        }
                    }
                }
            } else {
                stats.gaps += 1;
            }
            // THE PAIR'S OWN COMMAND IS THE NEXT PAIR'S PREDECESSOR.
            // This used to read `if let Some((_, _, c)) = &prev` AFTER
            // `prev.take()` had already emptied it — a dead lane, so
            // `prev_cmd` stayed frozen at its boundary seed and the
            // port's cast EDGE (`cmd.fire && !prev_fire`, world.rs)
            // degenerated to the raw HELD level for the whole run.
            // Retail's non-rapid spells (`byte_0x3B_59 == 1`, e.g.
            // possession) fire ONLY off the consumed press latch
            // (`HandleMouseButtons_18F80`, PlayerInput.cpp:2043-49 +
            // the frame-end latch clear at PI:1049-52) — one cast per
            // click — so the level trigger over-fired every hold.
            if !freeze_prev_cmd {
                prev_cmd = pcmd;
            }
        }
        prev = Some((tick.t, st, cmd));
        if let Some(limit) = args.limit {
            if stats.pairs >= limit {
                break;
            }
        }
    }
    print!("{}", stats.render(args, roster.as_ref()));
    print!("{}", pose_chan.render());
    if torn_rows > 0 {
        // Ordered by weight: the heaviest family is the one most
        // likely to be hiding something.
        let mut fam: Vec<_> = torn_fam.iter().collect();
        fam.sort_by_key(|(k, v)| (std::cmp::Reverse(**v), **k));
        println!(
            "   per-entity TEAR (fields excluded from grading, presence still compared): \
             {torn_rows} slot-exclusions across {torn_pairs} pairs, {} distinct slot(s)",
            torn_distinct.len()
        );
        for ((c, m), n) in fam.iter().take(8) {
            println!("     ({c:>3},{m:>3}): {n}");
        }
        if fam.len() > 8 {
            println!("     … and {} more (class, model)", fam.len() - 8);
        }
    }
    if let Some(sh) = shadow.as_ref() {
        // Pair mode's question is "which family is worst", so the
        // report keeps the map's own (class, model, lane) order.
        print!("{}", sh.render(false));
    }
    // Both counters cover the whole STREAM READ, not just `--start`'s
    // window: the input chain is fed from the first record read
    // regardless, and the ring lane is a "did this take ever trip it"
    // question. With `--start` the read begins at the warm-up point,
    // and the label says so instead of claiming the whole take.
    let counted = match skip_from {
        Some(s) => format!("from t={s}"),
        None => "whole stream".to_string(),
    };
    println!(
        "   cast input ({counted}): {press_moves} press-position move(s) [fold {}], \
         {ring_casts} cycle-ring (0x40) cast(s) [{}]",
        if press_edge_mode { "ON" } else { "off" },
        if ring_bit_off {
            "detector off"
        } else {
            "detector on"
        },
    );
    Ok(stats.clean_pairs == stats.pairs)
}

/// The recorded MC2 raw externals → the human's command. `fire = held
/// || latch`: the held registers mirror MC1's; the press LATCH is set
/// at the press edge and survives until release, so a click shorter
/// than one poll interval still registers. Takes without an input
/// channel yield the default (no casts).
pub(crate) fn sample_cmd_mc2(input: Option<&serde_json::Value>) -> PlayerCommand {
    let Some(i) = input else {
        return PlayerCommand::default();
    };
    let get = |obj: &str, key: &str| {
        i.get(obj)
            .and_then(|b| b.get(key))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    PlayerCommand {
        fire_left: get("mouse_buttons", "left") || get("mouse_clicks", "left"),
        fire_right: get("mouse_buttons", "right") || get("mouse_clicks", "right"),
        respawn: respawn_key_mc2(input),
        ..Default::default()
    }
}

// The respawn SPACE lane + press-dating witness laws moved to the
// shared recovery home (mgc_formats::recover — doc comments travel
// with them) so the app's `--replay` shares one implementation.
// (`mouse_pos` moved inside `Mc2RespawnWitness` with the shared dating
// rule — see the respawn lane above.)
pub(crate) use mgc_formats::recover::respawn_key as respawn_key_mc2;

/// The two recorded MC2 mouse registers, UNMERGED:
/// `((held_l, held_r), (latch_l, latch_r))` — the ISR held state
/// (`mouse_buttons`, `x_WORD_18074C`/`18074A`) and the one-shot press
/// LATCH (`mouse_clicks`, `x_WORD_180746`/`180744`).
pub(crate) fn raw_input_mc2(input: Option<&serde_json::Value>) -> ((bool, bool), (bool, bool)) {
    let Some(i) = input else {
        return ((false, false), (false, false));
    };
    let get = |obj: &str, key: &str| {
        i.get(obj)
            .and_then(|b| b.get(key))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    (
        (get("mouse_buttons", "left"), get("mouse_buttons", "right")),
        (get("mouse_clicks", "left"), get("mouse_clicks", "right")),
    )
}

/// THE CAST-PHASE LAW (session 9, ±1 cast-phase dig — ledger
/// §"THE RECORDER'S SNAPSHOT STRADDLES RETAIL'S INPUT POLL").
///
/// Retail's frame is `PlayerEvents` (input poll → `Turn++` → the cast
/// chain) → the entity pass → draw → the native limiter spin, and the
/// recorder parks in that settled tail: record `r`'s registers are read
/// AFTER frame `r`'s poll and BEFORE frame `r+1`'s. A press therefore
/// shows up in the recording either already consumed (frame `r` cast
/// it) or still pending (frame `r+1` will) — and retail says WHICH:
/// the press LATCH is set by the ISR and cleared the moment
/// `HandleMouseButtons_18F80` consumes it (PlayerInput.cpp:2043-49 +
/// the frame-tail drop at PI:1049-52), so a latch that is still up at
/// the snapshot means the poll has NOT run yet.
///
/// So the input frame `r` actually polled is
///
/// ```text
///   aligned(r) = (held(r) && !latch(r))   // already polled by frame r
///             || latch(r-1)               // pending at r-1 ⇒ frame r takes it
/// ```
///
/// and the pair `(r-1 → r)` — which IS frame `r`'s transition — is the
/// one that must carry it. MEASURED (probe over the raw states, no port
/// involved): retail's own arm records (the hand manifestation's
/// `word_0x2E_46` 0 → nonzero) land on an `aligned` RISING EDGE with
/// delta 0 on 403/404 mc2l4 right-hand casts, 412/412 mc2l0, 256/256
/// mc2l24, 39/39 + 54/57 + 36/36 left-hand. The raw held edge alone
/// splits 308/95 (mc2l4) between "same record" and "one record early" —
/// that split IS the latch bit, and no uniform `--input-delay` can
/// model it.
pub(crate) fn align_cmd_mc2(
    held: (bool, bool),
    latch: (bool, bool),
    prev_latch: (bool, bool),
) -> PlayerCommand {
    PlayerCommand {
        fire_left: (held.0 && !latch.0) || prev_latch.0,
        fire_right: (held.1 && !latch.1) || prev_latch.1,
        ..Default::default()
    }
}

pub(crate) use mgc_formats::recover::press_pos as press_pos_mc2;

/// `MGC_PRESS_EDGE=1` A/B lane: fold a cursor-AT-PRESS CHANGE into the
/// aligned rising edge, attributed to whichever button the record shows
/// down. The ISR writes the snapshot only on a press edge and nothing
/// ever clears it, so a change between two records proves a press
/// happened in between — including one that the poll both latched and
/// consumed inside the gap, which the latch lane cannot see.
///
/// MEASURED (mc2l0, full 8,626-pair take, against retail's own arm
/// oracle = the equipped hand manifestation's `word_0x2E_46` going
/// 0 → nonzero): 731 retail arms; the landed latch law catches
/// **728/731** with 201 armless edges, the press-position edge catches
/// **480/731** with **354** changes that arm nothing (UI clicks, the
/// ring pane, mana-refused casts, possess re-presses that raise
/// `byte_0x3C_60` instead of the timer). It is strictly worse, so it
/// stays OFF; the lane exists to keep that result reproducible and to
/// stand as the fallback if a recorder change ever costs us the latch.
pub(crate) fn press_edge_mc2(
    cmd: PlayerCommand,
    held: (bool, bool),
    latch: (bool, bool),
    moved: bool,
) -> PlayerCommand {
    if !moved {
        return cmd;
    }
    let (l, r) = (held.0 || latch.0, held.1 || latch.1);
    PlayerCommand {
        // Ambiguous records (the press already released, so no button
        // reads down) can only be attributed to a hand by guessing;
        // retail's own registers say nothing, so both fire.
        fire_left: cmd.fire_left || l || !(l || r),
        fire_right: cmd.fire_right || r || !(l || r),
        ..cmd
    }
}

/// THE CYCLE-RING CAST LANE (`entityIndex_0x6E3E_byte5 & 0x40`) —
/// the third cast bit beside the two hands, and the one the port does
/// not model.
///
/// PROVEN SEMANTICS. The carpet's dispatch tail fires three lanes off
/// `str_164->entityIndex_0x0` (EF:60851-62):
///
/// ```text
///   & 0x10 → sub_5F660(carpet, SpellEnabled[SpellIndexLeft ], 256)
///   & 0x20 → sub_5F660(carpet, SpellEnabled[SpellIndexRight], 512)
///   & 0x40 → sub_5F660(carpet,
///              SpellEnabled[spellIndex_D94FF[spellIndex_0x458_1112]], 256)
/// ```
///
/// So 0x40 is NOT "which hand" — it casts the RING PANE's category
/// cursor through the LEFT hand-slot flag (256), consulting neither
/// equipped hand. It is raised at exactly one site (PI:880-84): the
/// spell-ring pane (`MenuState` 5 or 8, PI:806) with no equip pending
/// (`byte_0x457_1111 == 0`, PI:836/842), no SHIFT (PI:856), and BOTH
/// press latches up (`MouseButtonState & 1 && & 2` — bits 0/1 are the
/// ISR latches `x_WORD_180746`/`180744`, EF:49676-79). The dispatcher
/// writes `spellIndex_0x458_1112 = byte1` first (EF:37626-27), so the
/// cast reads the cursor the click just selected — the shortcut that
/// casts a ring spell without equipping it.
///
/// ⚠⚠⚠ **THE "CORPUS: UNREACHABLE" VERDICT ABOVE WAS AN ARTEFACT OF
/// RE-DERIVING THE BIT FROM THE INPUT LATCHES.** Those PI conditions
/// are how the SENDER raises 0x40; the recording carries the RESULT.
/// `move_bits` IS `str_164->entityIndex_0x0`, the consumed command
/// word the two hand bits are already read off (`recover::mc1_fire`,
/// the "fire rides the consumed byte" law) — so the third bit needs no
/// reconstruction either, and the latch fold was answering a question
/// retail never asks at the cast site (`sub_5F380`'s tail is a bare
/// `testb $0x40`).
///
/// CORPUS, MEASURED OFF THE WORD: **one record in ten MC2 takes.**
/// mc2l22 t=63318 records `move_bits = 64` (0x10 and 0x20 both clear),
/// `menu_state = 5`, `hand_pending = 0`, `ring_cursor = 20` — and
/// retail arms the `(15,20)` token that tick (`f2e 0 -> 27`, `f30 =
/// 27`) while both hands hold spells 9 and 1, firing the `(9,22)` at
/// slot 816 on the next. That is mc2l22's LAST divergence head.
/// The pane/pending legs are kept as a sanity bound on the cursor;
/// they are corroboration, not the trigger. `MGC_NO_HAND_BIT=1`
/// disables the detector, `MGC_NO_MC2_RING_CAST_BIT=1` the sim lane.
pub(crate) fn ring_cast_mc2(p: &mgc_formats::mgcr::RetailPlayerMc2) -> Option<u8> {
    // `spellIndex_D94FF` (GameUI.cpp:59) is the identity over 0..25;
    // the three tail cells (26..28 → 0, 3, 0) are pane-layout padding
    // the cursor never lands on.
    mgc_formats::recover::mc2_ring_cast(p.move_bits, p.ring_cursor)
}

// The MC2 capture-grade law (module doc: step-1 dominance of the
// per-entity phase byte) moved to the shared recovery home.
pub(crate) use mgc_formats::recover::capture_clean_mc2;

/// One fixture-grade pair on a prepared MC2 world — the single
/// implementation behind both `verify-deltas` and the fixture suite.
///
/// Within an ACCEPTED pair, individual entities can still be torn:
/// the snapshot parks at a pass boundary, and a minority of entities
/// has already run 0 or 2 passes (phase delta ≠ 1). Their recorded
/// fields are capture artifacts — one decay/move step behind or
/// ahead — so they are excluded from FIELD comparison (presence still
/// compares). The corpus signature: perfectly balanced ± families
/// (life ±1, z ±64, speed ±4) that no sim law could produce.
#[allow(clippy::too_many_arguments)]
pub(crate) fn exec_pair_mc2(
    world: &mut World,
    pristine: &Planes,
    measured: Option<crate::verify::MeasuredPlanes<'_>>,
    things: &ThingTable,
    pst: &RetailMc2,
    st: &RetailMc2,
    obs: &ObsMc2,
    cmd: PlayerCommand,
    prev_cmd: PlayerCommand,
    phase: crate::verify::PairPose,
    // The recovered `mc2_park` for THIS pair — retail's
    // `PlayerEvents_51BB0` case 0x27 full stop, which the import
    // cannot see from the state at N alone
    // (`World::mc2_full_stop_import`).
    full_stop: bool,
) -> Result<
    (
        PairDiff,
        ObsMc2,
        mgc_sim::engine::world::conformance::ImportReport,
    ),
    String,
> {
    world.restore_planes(pristine);
    if let Some((h, ty, ceil, an)) = measured {
        world
            .install_measured_terrain(h, ty, ceil, an)
            .map_err(|e| format!("terrain: {e}"))?;
    }
    world.restore_thing_table(things);
    let report = world
        .retail_import_mc2(pst)
        .map_err(|e| format!("import: {e}"))?;
    world.mc2_full_stop_import(pst, full_stop);
    world.set_prev_fire(prev_cmd.fire_left, prev_cmd.fire_right);
    // The barrel roll's homing-lock break (`sub_55EB0`) — retail's
    // PLAYER FRAME fires it before `UpdateEntities_57730`, and the
    // replay lane pins the carpet instead of running the driver, so
    // the edge has to come off the imported roll phase. See
    // [`World::mc2_broll_lock_break`].
    world.mc2_broll_lock_break(
        pst.players[pst.local_player as usize].broll_phase,
        st.players[st.local_player as usize].broll_phase,
    );
    // The POSE PAIR ([`mc2_pose_pair`]): feed the walk BOTH recorded
    // endpoints and let `mc2_carpet_dispatch` swap them at the
    // carpet's own walk slot, retail's `sub_5D530` phase. The
    // PROJECTION pin stays on N+1 either way (the recorded
    // observation IS the settled pose) — `crate::verify::exec_pair`'s
    // law verbatim.
    let pre = carpet_pose_mc2(&pst.ents[report.human_slot as usize]);
    let post = carpet_pose_mc2(&st.ents[report.human_slot as usize]);
    let pose = if matches!(phase, crate::verify::PairPose::PinN) {
        pre
    } else {
        post
    };
    if matches!(phase, crate::verify::PairPose::Pair) {
        world.tick_pose_pair(pre, post, cmd);
    } else {
        world.tick(pose, cmd);
    }
    let mut castles = [0i16; 8];
    for (i, p) in pst.players.iter().take(8).enumerate() {
        castles[i] = p.castle;
    }
    let pin = PinnedMc2 {
        slot: report.human_slot,
        local: pst.local_player,
        player_count: pst.player_count,
        pose,
        castles,
    };
    let port = world.obs_project_mc2(&pin);
    let torn = torn_slots_scoped(pst, st, TearScope::Graded);
    let mut pd = compare_mc2_gated(obs, &port, report.human_slot, &torn);
    append_sprite_diffs_mc2(&mut pd, st, world, report.human_slot, &torn);
    Ok((pd, port, report))
}

/// The MC2 half of the SPRITE lane (see verify.rs
/// `append_sprite_diffs`): retail `f5a` vs port `type86`, gated the
/// way every MC2 comparison is — torn slots skipped, class/model must
/// agree (a slot holding different entities is the graded diff's own
/// missing/extra story).
pub(crate) fn append_sprite_diffs_mc2(
    pd: &mut PairDiff,
    st: &RetailMc2,
    world: &World,
    human_slot: u16,
    torn: &std::collections::BTreeSet<u16>,
) {
    for (slot, ty, class, model) in world.sprite_lane() {
        if slot == human_slot || torn.contains(&slot) {
            continue;
        }
        let Some(e) = st.ents.get(slot as usize) else {
            continue;
        };
        if e.class3f != class || e.model40 != model {
            continue;
        }
        if e.f5a as u16 != ty {
            pd.fields.push(FieldDiff {
                slot: Some(slot),
                field: "f5a",
                want: (e.f5a as u16).to_string(),
                got: ty.to_string(),
            });
        }
    }
}

/// ⭐ A HELD PHASE BYTE IS NOT A CAPTURE TEAR — IT IS THE SIGNATURE OF
/// A NULL-DISPATCH ACTION. `UpdateEntities_57730` bumps
/// `byte_0x3E_62` only INSIDE the arm that actually ran a handler
/// (EF:40172, guarded by `str_D4C48ar[class].dword_10[action].word_4
/// == action` and `.dword_10 != 0`), so an entity parked in an action
/// whose dispatch row is disabled never ticks and its phase byte rests
/// at the spawn stamp (`NewEvent_4A050`, Events.cpp:577 — the byte is
/// seeded to the slot index) for the entity's whole life.
///
/// This is the complete no-bump set, read off the SHIPPED action
/// tables `x_DWORD_D4C52ar_str<class>0` (EventsFunctions.cpp:1146-2030;
/// row = `{dword_0, word_4, address_6, dword_10}`, EventsFunctions.h:223):
/// a row is no-bump iff `word_4 != action || dword_10 == 0`. Every
/// other class's only such row is its array TERMINATOR, which no
/// entity ever holds, so only classes 5 and 10 appear here.
///   class 5:  0x28-0x47, 0x58-0x5F (real addresses, `dword_10 == 0`),
///             0xEA — the m27 TIER-2 SEGMENT (EF:1477; the 9-per-branch
///             spline bodies, moved only by the head's `sub_2AA90`).
///   class 10: 0x27/0x2F/0x31/0x32 (`word_4 == 0`), 0x2E, 0x3F
///             (`word_4 == 0x29`), 0x52, 0x54 — 0x54 (EF:1686) is the
///             (10,77) FIRE-SPHERE SATELLITE, 0x52 (EF:1684) its
///             (10,75) sibling; both are moved only by their hub.
///
/// ⚠ class-5 0xE9 (the m27 BRANCH) is a null row too and is
/// deliberately NOT listed: the body's `sub_29A90` bumps it out of
/// band on every tick (EF:19804), and the recording agrees — 191,695
/// of 191,695 `(5,27)` action-233 pairs on mc2l22 advance by exactly
/// 1. It stays under the +1 test.
///
/// ⭐⭐⭐ **ARMED BY DEFAULT SINCE ROUND 149 — WITH THE PROOF SESSION 96
/// OWED.** The citation above was written in session 96 and then left
/// unmeasured for fifty rounds; round 149 (dig w149j) measured the
/// ON/OFF pair on both focus takes and both halves came back FREE:
///
/// | take   | arm      | tear-gate hidden | RAW SHADOW mismatches | graded |
/// |--------|----------|------------------|-----------------------|--------|
/// | mc2l24 | legacy   |        1,936,794 |  980 (4 lanes)        | 10 seg |
/// | mc2l24 | class 5  |          345,999 |  980 (same 4 lanes)   | 10 seg |
/// | mc2l24 | 5 + 10   |          269,714 |  980 (same 4 lanes)   | 10 seg |
/// | mc2l24 | + reseed |            3,309 |  980 (same 4 lanes)   | 10 seg |
/// | mc2l22 | legacy   |        2,298,158 | 1307 (3 lanes)        |  4 seg |
/// | mc2l22 | class 5  |          572,903 | 1307 (same 3 lanes)   |  4 seg |
/// | mc2l22 | 5 + 10   |          477,525 | 1307 (same 3 lanes)   |  4 seg |
/// | mc2l22 | + reseed |           12,472 | 1308 (+1 row, old lane)|  4 seg |
///
/// 3.28 M slot-boundaries that had NEVER been shadowed on any ungraded
/// lane came back BYTE-CLEAN, and `--segmented --brief` is unchanged on
/// both takes. ⚠ The class-10 warning below is now HISTORY: the (10,77)
/// fire-sphere wall it describes (673 → 1,935 dirty pairs, 5,219 →
/// 306,813 rows on mc2l22) has been PAID by the constellation-tumble
/// laws that landed after session 96 — the same flag on today's tree
/// moves nothing. ⚠ `MGC_TEAR_PHASE_LAW` and `MGC_TEAR_NO_BUMP_C10`
/// are RETIRED (the law they armed is the default); old recipes that
/// set them now get the default arm either way, and the only switch is
/// `MGC_TEAR_LEGACY=1`.
///
/// ⭐ THE CADENCE, MEASURED OFF THE RECORDING (round 149, whole
/// mc2l24, consecutive recorded ticks, wrapping delta of `phase3e`):
///   (5,27) act 0xEA: +0 ×1,591,335 / +1 ×135    — null row, HELD
///   (5,27) act 0xE9: +1 ×176,770  / +0 ×60      — null row BUT the
///                    body's `sub_29A90` bumps it out of band (EF:19827)
///   (10,75) act 0x52: +0 ×28,006  (100.0%)      — null row, HELD
///   (10,77) act 0x54: +0 ×49,875  / −17 ×8      — null row, HELD
/// Every one of those four is exactly what the shipped table predicts
/// (`address_6 == 0 && dword_10 == 0` for 0xE9/0xEA/0x52/0x54).
///
/// ⚠ **THE TEST IS STILL WRONG FOR THE RE-SEED SPECIES** — see
/// [`torn_slots`]'s `MGC_TEAR_RESEED_LAW`.
///
/// Set `MGC_TEAR_LEGACY=1` to restore the bare `!= 1` test.
fn mc2_no_bump_action(class: u8, action: u8) -> bool {
    match class {
        5 => matches!(action, 0x28..=0x47 | 0x58..=0x5F | 0xEA),
        // HISTORY (session 96, superseded round 149): "un-excluding
        // the class-10 rows UNCOVERS A WALL rather than confirming
        // conformance: on mc2l22 the (10,77) FIRE-SPHERE SATELLITES
        // (action 0x54) alone take the census from 673 to 1935 dirty
        // pairs and 5,219 to 306,813 unexplained field rows —
        // x/y/z/pitch on 387 satellite slots over 1,287 ticks, the
        // constellation tumble (`sub_33B20`, mc2/tail.rs) drifting out
        // of phase." That debt is PAID: round 149 measured mc2l22 with
        // this half armed at 1,307 raw-shadow mismatches, identical to
        // the legacy arm, and 95,380 fewer exclusions.
        10 => matches!(
            action,
            0x27 | 0x2E | 0x2F | 0x31 | 0x32 | 0x3F | 0x52 | 0x54
        ),
        _ => false,
    }
}

/// The bare `phase3e != 1` tear test, as it stood before round 149.
/// `MGC_TEAR_LEGACY=1` — the kill switch for the cadence law.
fn tear_legacy() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var_os("MGC_TEAR_LEGACY").is_some())
}

/// ⚠ OPT-IN, ROUND 149 — **A RE-SEEDED PHASE BYTE IS NOT A TEAR
/// EITHER, AND A TEAR CANNOT MOVE A SLOT BY MORE THAN ONE DISPATCH.**
///
/// `byte_0x3E_62` is not only bumped by `UpdateEntities_57730`; it is
/// RE-SEEDED at birth from a per-model round-robin counter,
/// `a1x->byte_0x3E_62 = D41A0_0.array_0x10[a1x->model_0x40_64]++`
/// (EF:20780 `sub_2AC50`//20bc50, EF:19172 `sub_28CE0`//209ce0, and
/// the whole `NewEvent` family EF:33792/33836/33883/33952/33996/34040/
/// 34079/34114/34171/34202/…). A slot that is freed and re-allocated
/// to the SAME (class, model) inside one capture interval therefore
/// shows an arbitrary delta that no tear could produce.
///
/// A capture tear can only shift a slot by ONE dispatch on each side
/// of the pair, so for a +1-per-dispatch counter the tear-reachable
/// deltas are exactly {−1, 0, +1, +2}. Anything else — mc2l24's
/// (10,0) `−10 ×51,753 / −9 ×15,966 / −6 ×2,737 / −5 ×2,591` and
/// (10,14) `−16/−17/−31/…` families — is a re-seed and MUST be graded.
///
/// ⚠⚠ This deliberately WIDENS grading, so it ships with its own
/// measurement, per ⭐⭐⭐ A GRADING ORACLE TUNED TO ITS OWN SCORE CAN
/// HIDE A PORT BUG. Round 149, on top of the no-bump law (both
/// clauses — the out-of-range delta and the life-rose witness):
///   mc2l24  hidden 269,714 → **3,309** · RAW SHADOW 980 → **980**
///           (the same 4 lanes) · `--brief` byte-identical
///   mc2l22  hidden 477,525 → **12,472** · RAW SHADOW 1,307 → **1,308**
///           (the one new row lands in the EXISTING `(5,16) roll`
///           lane, 1,303 → 1,304 rows, same 3 slots, same t window —
///           a row the gate had been hiding, not a new family)
/// ⚠ The MC1/HW half of this law was already written down —
/// docs/RECORDING.md "Capture tearing": *"only steps of exactly dv±1
/// count as tear suspects … arbitrary-step deviants are ambient spawn
/// CHURN (slot re-use overwrites +63 with the spawn ordinal)"* — and
/// `capture_clean_mc2`'s own PAIR test already ignores deltas outside
/// {0,1,2}. Only this per-slot gate never got it: ONE LADDER, TWO
/// COPIES, ONE WRONG.
/// `MGC_TEAR_NO_RESEED=1` disarms this half alone; `MGC_TEAR_LEGACY=1`
/// disarms the whole cadence law.
/// `MGC_TEAR_NO_SLOT_SEED=1` — the kill switch for the
/// `NewEvent_4A050` slot-index seed clause of [`slot_is_torn`]
/// (round 150). It rides inside [`tear_reseed_law`], so
/// `MGC_TEAR_NO_RESEED=1` and `MGC_TEAR_LEGACY=1` disarm it too.
fn tear_slot_seed_law() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var_os("MGC_TEAR_NO_SLOT_SEED").is_none())
}

fn tear_reseed_law() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var_os("MGC_TEAR_NO_RESEED").is_none())
}

/// Slots live at both ends whose phase byte did NOT advance exactly
/// once — per-entity capture tear inside an accepted pair.
pub(crate) fn torn_slots(pst: &RetailMc2, st: &RetailMc2) -> std::collections::BTreeSet<u16> {
    torn_slots_scoped(pst, st, TearScope::Shadow)
}

/// Which consumer the tear verdict is for.
///
/// HISTORY (round 149 → 150). The re-seed clauses of [`slot_is_torn`]
/// shipped SHADOW-ONLY, because on the GRADED pair diff they un-hid
/// the fixture `mc2l16/a-dead-carpet-is-not-on-the-switch-fire-roster`
/// (t=7903): four `(10,0)`/`(10,1)` death-fall newborns — one of them
/// slot 934, REVIVED mid-tick — placed at retail's 7065 and the
/// port's 6943. Round 150 dug that row: it was not a tear and not a
/// gate defect but the pinned-pair arm of the death fall reading the
/// SETTLED z where retail reads the mover's pre-gravity output
/// (`no_mc2_fall_puff_pinned_pre_z`). With that landed, the widened
/// graded scope is clean, so **both consumers now take the full
/// cadence law** — see [`slot_is_torn_scoped`]. The enum is kept as
/// the lever for the next widening.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TearScope {
    /// The graded pair diff and the fixture suite.
    Graded,
    /// The raw-shadow census.
    Shadow,
}

pub(crate) fn torn_slots_scoped(
    pst: &RetailMc2,
    st: &RetailMc2,
    scope: TearScope,
) -> std::collections::BTreeSet<u16> {
    let mut torn = std::collections::BTreeSet::new();
    for slot in 1..pst.ents.len().min(st.ents.len()) {
        let (a, b) = (&pst.ents[slot], &st.ents[slot]);
        if a.class3f == 0 || a.class3f != b.class3f || a.model40 != b.model40 {
            continue;
        }
        if slot_is_torn_scoped(slot, a, b, scope) {
            torn.insert(slot as u16);
        }
    }
    torn
}

/// The per-slot half of [`torn_slots`], pulled out so the cadence law
/// is unit-testable without a 168-byte×1000 closure.
#[cfg(test)]
fn slot_is_torn(a: &RetailEntMc2, b: &RetailEntMc2) -> bool {
    // Slot 0 is the reserved hole, so the slot-seed clause below can
    // never fire for it — the unit tests keep testing the cadence
    // clauses in isolation. `slot_is_torn_at` is the seeded form.
    slot_is_torn_scoped(0, a, b, TearScope::Shadow)
}

/// [`slot_is_torn`] at a named slot, for the `NewEvent_4A050`
/// slot-index seed clause.
#[cfg(test)]
fn slot_is_torn_at(slot: usize, a: &RetailEntMc2, b: &RetailEntMc2) -> bool {
    slot_is_torn_scoped(slot, a, b, TearScope::Shadow)
}

fn slot_is_torn_scoped(
    slot: usize,
    a: &RetailEntMc2,
    b: &RetailEntMc2,
    scope: TearScope,
) -> bool {
    // ⭐ ROUND 150 (w150a): THE RE-SEED CLAUSES ARE GRADED NOW. They
    // were `TearScope::Shadow`-only because widening them un-hid
    // mc2l16 t=7903 — four death-fall newborns the pinned-pair arm
    // planted one gravity step low. That row is DUG AND LANDED
    // (`no_mc2_fall_puff_pinned_pre_z`), and with the law on, the
    // widened graded scope reads clean: 596/596 fixtures over every
    // mc1*/mc2* manifest, and mc2l16 + mc2l22 `--segmented --brief`
    // byte-identical (both `segments=1 devs=0 horizon=END`). The
    // scope stays in the signature as the lever for the next
    // widening; today both consumers take the same verdict.
    let _ = scope;
    let reseed = tear_reseed_law();
    let d = b.phase3e.wrapping_sub(a.phase3e);
    if d == 1 {
        return false;
    }
    if tear_legacy() {
        return true;
    }
    // A HELD phase byte under an action that retail never dispatches
    // is the entity's normal cadence, not a capture tear — see
    // `mc2_no_bump_action`. Excluding it blinded the census to whole
    // families for whole takes (mc2l22: 45 m27 tier-2 segments,
    // 1,725,255 of the take's 2,298,158 slot-exclusions, ungraded for
    // 38,340 consecutive pairs).
    if a.phase3e == b.phase3e
        && a.action45 == b.action45
        && mc2_no_bump_action(a.class3f, a.action45)
    {
        return false;
    }
    // A tear shifts a slot by at most ONE dispatch per side, so
    // {−1, 0, +1, +2} is the whole tear-reachable set for a
    // +1-per-dispatch counter. A wilder delta is a BIRTH re-seed
    // (`array_0x10[model]++`) — see `tear_reseed_law`.
    if reseed && !matches!(d, 0xFF | 0 | 1 | 2) {
        return false;
    }
    // ⭐⭐⭐ …AND THE EXACT NEWBORN WITNESS, WHICH THE RECORD HAS
    // CARRIED ALL ALONG: `NewEvent_4A050` seeds `byte_0x3E_62` TO THE
    // RECORD'S OWN SLOT INDEX. Shipped `NETHERW.EXE` file 0x6E850
    // (= VA 0x4A050 + 0x24800), the ctor tail:
    //   6e913: 89 d8              mov  eax,ebx        ; the record
    //   6e915: 8d 91 8e 6e 00 00  lea  edx,[ecx+0x6e8e] ; the pool base
    //   6e91b: 29 d0              sub  eax,edx
    //   6e91f: be a8 00 00 00     mov  esi,0xa8       ; the 168-byte stride
    //   6e927: f7 fe              idiv esi            ; ⭐ eax = SLOT INDEX
    //   6e946: 66 89 43 1a        mov  [ebx+0x1a],ax  ; …into @0x1A too
    //   6e976: 88 43 3e           mov  [ebx+0x3e],al  ; ⭐⭐⭐ THE SEED
    // ⛔ ROUND 149's `array_0x10[model]++` IS THE CLASS-5 CREATURE
    // CTOR'S counter (29 models, `Gen::mc2_ord`) and governs NONE of
    // the class-9/10 effect families this gate trips over. Measured
    // over the whole of mc2l22, every non-`+1` live pair cross-tabbed
    // against both tests:
    //   array_0x10[model] in [pre, post)   465,052 heur-only,      0 ord-only
    //   phase == slot (or slot+1)          465,050 BOTH,      12,471 ord-only,
    //                                            3 heur-only,     1 neither
    // — the 12,471 are exactly the round-149 residue ((9,9) 7,903,
    // (9,13) 2,762, (9,0) 1,711, (10,11) 94), and the gate's leftover
    // drops from 12,472 slot-exclusions to ONE. The `slot + 1`
    // alternative is the newborn that also took its own dispatch in
    // the walk it was minted in (mc2l16 t=7903 slot 934: minted 166,
    // recorded 167 — the four siblings minted above the cursor record
    // their bare slot byte).
    // `MGC_TEAR_NO_SLOT_SEED=1` disarms this clause alone.
    if reseed && tear_slot_seed_law() {
        let si = slot as u8;
        if b.phase3e == si || b.phase3e == si.wrapping_add(1) {
            return false;
        }
    }
    // …and the INDIRECT re-seed witness: retail's `life_0x8` counts DOWN
    // under the handler (`sub_67410`//248410 is literally
    // `life--; if (old < 0) retire`), so a life that ROSE across the
    // pair is a slot that was freed and re-allocated inside the
    // interval — a new instance, with a new `array_0x10[model]` seed.
    // A capture tear cannot raise a countdown. mc2l24's (9,9) sparks
    // are a 2-tick species that respawns in place; cross-tabulated
    // over the whole take, EVERY +1 pair (278,523) has life falling
    // and 179,457 of the 180,248 non-+1 pairs have life RISING.
    if reseed && b.life > a.life {
        return false;
    }
    true
}

/// The recorded carpet's raw fields as the pinned pose. MC2's live
/// facing is the WORLD yaw @0x1C (the applied yaw @0x52 rests at a
/// constant for the player — see the recorder field map).
pub(crate) fn carpet_pose_mc2(e: &RetailEntMc2) -> PlayerPose {
    PlayerPose {
        x: e.x,
        y: e.y,
        z: e.z,
        heading: e.yaw as u16,
        pitch: e.pitch as u16,
        speed: e.speed,
    }
}

/// The MC2 world recipe — the app's `WorldInit::build` MC2 arm,
/// parameterized by level. The bundle variant follows the app's
/// header law (night-fog/night/cave/day).
pub(crate) fn build_world_mc2(
    baked: &std::path::Path,
    level: u32,
    replayed: bool,
) -> Result<(World, Planes, ThingTable), String> {
    build_world_mc2_with_book(baked, level, replayed, None)
}

/// [`build_world_mc2`] with the human's level-start BOOK (spell ids)
/// granted between the world ctor and the rival spawn — the slot
/// position retail's `sub_5C950` gives the carried book, so a native
/// world's pool lays out like the take's record 0 (`terrain-check`
/// reads the list off the take). `None` keeps the ctor's `{0, 1}` floor.
pub(crate) fn build_world_mc2_with_book(
    baked: &std::path::Path,
    level: u32,
    replayed: bool,
    human_book: Option<&[u8]>,
) -> Result<(World, Planes, ThingTable), String> {
    let lp = baked.join("mc2").join(format!("level-{level:03}.mgcl"));
    let file = std::fs::File::open(&lp).map_err(|e| format!("{}: {e}", lp.display()))?;
    let pkg: mgc_formats::LevelPackage =
        mgc_formats::mgcl::read(file).map_err(|e| format!("{}: {e}", lp.display()))?;
    if let Some(ov) = &pkg.meta.overlay {
        return Err(format!(
            "{}: MODDED level (overlay {ov}) — conformance runs against pristine bakes \
             only; delete baked/ and rebake without gamedata/overlay/ (docs/MODDING.md)",
            lp.display()
        ));
    }
    let header = pkg.header.as_ref();
    let variant = match header.map(|h| (h.map_type, h.gfx_type)) {
        Some((mgc_formats::MapType::Night, g)) if g & 2 != 0 => "mc2-night-fog",
        Some((mgc_formats::MapType::Night, _)) => "mc2-night",
        Some((mgc_formats::MapType::Cave, _)) => "mc2-cave",
        _ => "mc2-day",
    };
    let bundle = mgc_formats::bundle::Bundle::load(&baked.join("assets").join(variant))
        .map_err(|e| format!("bundle {variant}: {e}"))?;
    let terrain = pkg.terrain.as_ref().ok_or("package has no terrain")?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().ok_or("no shading plane")?,
        angle: terrain.angle.clone().ok_or("no angle plane")?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref().ok_or("bundle: no search data")?,
        bundle.build_tab.as_ref().ok_or("bundle: no build tab")?,
        bundle.build_dat.as_ref().ok_or("bundle: no build dat")?,
    )?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    // Day-sourced extents whatever the render variant — retail's
    // particle-param table is computed once at boot against TMAPS0-0
    // (Bundle::mc2_extent_dims holds the law; sprite 96's 38-vs-36
    // width is the dwelling f80 194-vs-184 family).
    if let Some(dims) = bundle.mc2_extent_dims(&baked.join("assets")) {
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp)?;
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let night = matches!(
        header.map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    );
    // The environment goes in at CONSTRUCTION (round 110): the load
    // settle's repaints read it inside `new_full`; the setter below
    // still owns the SPELLS rows-4/19 patch and the runtime repaints.
    let mut w = World::new_for_game_env(
        planes,
        &pkg.things.things,
        seed,
        assets,
        mgc_sim::ids::GameId::Mc2,
        night,
    );
    w.set_placeholders(true);
    w.set_mc2_night_shade(night);
    w.set_mc2_doom_level(header.is_some_and(|h| h.gfx_type & 2 != 0));
    w.set_mc2_castle_purge_level(header.is_some_and(|h| h.gfx_type & 4 != 0));
    w.set_mc2_level_replayed(replayed);
    if let Some(stages) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = stages
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        if !rows.is_empty() {
            w.set_mc2_stages(&rows);
        }
        let vars: Vec<(i8, i8, u8, u8, u32)> = stages
            .variables
            .iter()
            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
            .collect();
        if !vars.is_empty() {
            w.set_mc2_stagevars(&vars);
        }
    }
    if let Some(book) = human_book {
        w.mc2_grant_start_book(book);
    }
    let (wizards, player_count) = mc2_rival_configs(pkg.wizards.as_ref(), header);
    w.set_mc2_wizards(&wizards, player_count);
    let pristine = w.planes_clone();
    // The authored THING records: a one-shot disposition ZEROES the
    // records it releases, and that consumption is not part of the
    // captured `D41A0_0` closure — so it must be re-imprinted per
    // pair alongside the terrain, or one mis-timed trip disarms the
    // disposition for the whole rest of the run.
    let things = w.thing_table_clone();
    Ok((w, pristine, things))
}

/// wizards.json + header → per-color MC2 rival configs (the app's
/// resolver, same duplication the MC1 arm carries).
fn mc2_rival_configs(
    wizards: Option<&mgc_formats::Wizards>,
    header: Option<&mgc_formats::LevelHeader>,
) -> ([Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8] = Default::default();
    let (Some(w), Some(h)) = (wizards, header) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(reflexes), Some(perception)) = (cfg.reflexes, cfg.perception) else {
            continue;
        };
        let mut start = [false; 26];
        let mut start_level = [0u8; 26];
        let mut blocked = [false; 26];
        for s in 0..26 {
            start[s] = cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
            start_level[s] = cfg
                .starting_spell_levels
                .get(s)
                .copied()
                .unwrap_or(0)
                .min(2);
            blocked[s] = cfg.blocked_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(mgc_sim::mc2::rivals::Mc2RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            perception: perception.clamp(0, 255) as u8,
            reflexes: reflexes.clamp(0, 255) as u8,
            life: cfg.life.unwrap_or(0).max(0) as u16,
            castle_level: h.players[slot].max(0) as u8,
            start,
            start_level,
            blocked,
        });
    }
    (out, count)
}

// ------------------------------------------------------------- comparison

macro_rules! cmp_field {
    ($out:expr, $slot:expr, $name:literal, $want:expr, $got:expr) => {
        if $want != $got {
            $out.fields.push(FieldDiff {
                slot: $slot,
                field: $name,
                want: format!("{:?}", $want),
                got: format!("{:?}", $got),
            });
        }
    };
}

/// Field-aware MC2 obs comparison. Policy (mirrors the MC1 rules):
/// - the pinned human slot compares presence + life/mana only (its
///   pose fields are runner INPUTS, not predictions);
/// - `applied_yaw`/`applied_pitch` on the human are skipped for the
///   same reason (control-written);
/// - player `turn` is skipped (a frame counter the port does not
///   model — continuity, not gameplay state);
/// - player `flight` is skipped (input-reconstruction domain);
/// - entity `rand` IS compared (the per-entity u16 LCG stream is
///   sim state, same as MC1);
/// - `torn` slots (per-entity capture tear) compare presence only.
pub(crate) fn compare_mc2_gated(
    retail: &ObsMc2,
    port: &ObsMc2,
    human_slot: u16,
    torn: &std::collections::BTreeSet<u16>,
) -> PairDiff {
    let mut out = PairDiff {
        rng_want: retail.rng,
        rng_got: port.rng,
        ..Default::default()
    };
    let rmap: BTreeMap<u16, &EntObsMc2> = retail.entities.iter().map(|e| (e.slot, e)).collect();
    let pmap: BTreeMap<u16, &EntObsMc2> = port.entities.iter().map(|e| (e.slot, e)).collect();
    for (slot, re) in &rmap {
        let Some(pe) = pmap.get(slot) else {
            out.missing.push((*slot, re.class, re.model));
            continue;
        };
        if torn.contains(slot) {
            continue;
        }
        let s = Some(*slot);
        if *slot == human_slot {
            cmp_field!(out, s, "life", re.life, pe.life);
            cmp_field!(out, s, "mana", re.mana, pe.mana);
            cmp_field!(out, s, "mana_max", re.mana_max, pe.mana_max);
            continue;
        }
        cmp_field!(out, s, "class", re.class, pe.class);
        cmp_field!(out, s, "model", re.model, pe.model);
        cmp_field!(out, s, "life", re.life, pe.life);
        cmp_field!(out, s, "max_life", re.max_life, pe.max_life);
        cmp_field!(out, s, "x", re.x, pe.x);
        cmp_field!(out, s, "y", re.y, pe.y);
        cmp_field!(out, s, "z", re.z, pe.z);
        // Class-15 manifestations repurpose the world-yaw lane (@0x1C)
        // for the subSpellIndex payload (f30), so the port has no field
        // for a manifestation's facing and `obs_project_mc2` projects
        // heading 0 (conformance.rs). Retail's recorded obs still
        // carries the real facing: a DETACHED spell jar (model 0,
        // action 78) rests at its fling yaw indefinitely (mc2l24 slot
        // 73 holds ~1634 for 20k ticks; 25,334 class-15 heading rows in
        // that take, port 0 in all but 4). The facing is cosmetic —
        // cast direction reads f30/f34, never the world yaw — so it is
        // UNMODELED, not a prediction miss. The port's one-sided zeroing
        // intended this exclusion but the recorded side kept the value;
        // skip class-15 here to complete it (twin of the human
        // applied_yaw/applied_pitch skip). Other classes' heading is a
        // live motion prediction and stays compared.
        if re.class != 15 {
            cmp_field!(out, s, "heading", re.heading, pe.heading);
        }
        cmp_field!(out, s, "pitch", re.pitch, pe.pitch);
        cmp_field!(out, s, "applied_yaw", re.applied_yaw, pe.applied_yaw);
        cmp_field!(out, s, "applied_pitch", re.applied_pitch, pe.applied_pitch);
        cmp_field!(out, s, "speed", re.speed, pe.speed);
        cmp_field!(out, s, "mana", re.mana, pe.mana);
        cmp_field!(out, s, "mana_max", re.mana_max, pe.mana_max);
        cmp_field!(out, s, "owner", re.owner, pe.owner);
        cmp_field!(out, s, "action", re.action, pe.action);
        cmp_field!(out, s, "sv1", re.sv1, pe.sv1);
        cmp_field!(out, s, "sv2", re.sv2, pe.sv2);
        cmp_field!(
            out,
            s,
            "player_ent_idx",
            re.player_ent_idx,
            pe.player_ent_idx
        );
        cmp_field!(out, s, "rand", re.rand, pe.rand);
    }
    for (slot, pe) in &pmap {
        if !rmap.contains_key(slot) {
            out.extra.push((*slot, pe.class, pe.model));
        }
    }
    for (rp, pp) in retail.players.iter().zip(&port.players) {
        let s = None;
        match rp.index {
            0 => {
                cmp_field!(out, s, "player0.play_index", rp.play_index, pp.play_index);
                cmp_field!(out, s, "player0.castle", rp.castle, pp.castle);
                cmp_field!(out, s, "player0.hand_left", rp.hand_left, pp.hand_left);
                cmp_field!(out, s, "player0.hand_right", rp.hand_right, pp.hand_right);
            }
            _ => {
                cmp_field!(out, s, "rival.play_index", rp.play_index, pp.play_index);
                cmp_field!(out, s, "rival.castle", rp.castle, pp.castle);
            }
        }
    }
    if let (Some(rp), Some(pp)) = (&retail.player, &port.player) {
        let s = None;
        cmp_field!(out, s, "player.life", rp.life, pp.life);
        cmp_field!(out, s, "player.mana", rp.mana, pp.mana);
        cmp_field!(out, s, "player.mana_max", rp.mana_max, pp.mana_max);
        cmp_field!(out, s, "player.castle", rp.castle, pp.castle);
    }
    out
}

/// One TSV row per diff event (same shape as the MC1 emitter).
fn emit_csv_mc2(
    w: &mut impl std::io::Write,
    t: u64,
    pd: &PairDiff,
    retail: &ObsMc2,
    port: &ObsMc2,
    roster: Option<&crate::roster::Roster>,
    tags: Option<&crate::roster::RuleTags>,
) -> std::io::Result<()> {
    let rmap: BTreeMap<u16, &EntObsMc2> = retail.entities.iter().map(|e| (e.slot, e)).collect();
    let pmap: BTreeMap<u16, &EntObsMc2> = port.entities.iter().map(|e| (e.slot, e)).collect();
    // One rng row per pair (even when equal) — offline solvers need
    // the full retail stream, not just the mismatches.
    writeln!(w, "{t}\trng\t\t\t\t\t{}\t{}\t\t\t\t", retail.rng, port.rng)?;
    let ctx = |slot: u16| -> (String, String, String) {
        match rmap.get(&slot).or_else(|| pmap.get(&slot)) {
            Some(e) => (format!("{}", e.x), format!("{}", e.y), e.z.to_string()),
            None => Default::default(),
        }
    };
    let rule_id =
        |lane: fn(&crate::roster::RuleTags) -> &Vec<crate::roster::Tag>, i: usize| -> &str {
            match tags {
                Some(tg) => match lane(tg)[i] {
                    crate::roster::Tag::Rule(k) => roster.map_or("", |r| r.rules[k].id.as_str()),
                    crate::roster::Tag::PosePhase => "pose-phase",
                    crate::roster::Tag::SlotDesync => "slot-desync",
                    crate::roster::Tag::TerrainShadow => "terrain-shadow",
                    crate::roster::Tag::Unexplained => "",
                },
                None => "",
            }
        };
    for (i, (slot, c, m)) in pd.missing.iter().enumerate() {
        let (x, y, z) = ctx(*slot);
        let rid = rule_id(|t| &t.missing, i);
        writeln!(
            w,
            "{t}\tmissing\t{slot}\t{c}\t{m}\t\t\t\t{x}\t{y}\t{z}\t{rid}"
        )?;
    }
    for (i, (slot, c, m)) in pd.extra.iter().enumerate() {
        let (x, y, z) = ctx(*slot);
        let rid = rule_id(|t| &t.extra, i);
        writeln!(
            w,
            "{t}\textra\t{slot}\t{c}\t{m}\t\t\t\t{x}\t{y}\t{z}\t{rid}"
        )?;
    }
    for (i, d) in pd.fields.iter().enumerate() {
        let rid = rule_id(|t| &t.fields, i);
        match d.slot {
            Some(slot) => {
                let (c, m) = rmap
                    .get(&slot)
                    .or_else(|| pmap.get(&slot))
                    .map_or((0, 0), |e| (e.class, e.model));
                let (x, y, z) = ctx(slot);
                writeln!(
                    w,
                    "{t}\tfield\t{slot}\t{c}\t{m}\t{}\t{}\t{}\t{x}\t{y}\t{z}\t{rid}",
                    d.field, d.want, d.got
                )?;
            }
            None => writeln!(
                w,
                "{t}\tfield\t\t\t\t{}\t{}\t{}\t\t\t\t{rid}",
                d.field, d.want, d.got
            )?,
        }
    }
    Ok(())
}

/// Slot → (class, model) map for the family-neutral signature builder.
pub(crate) fn class_map_mc2(retail: &ObsMc2) -> BTreeMap<u16, (u8, u8)> {
    retail
        .entities
        .iter()
        .map(|e| (e.slot, (e.class, e.model)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One recorded input frame: held + latch, as the recorder writes
    /// them (`mouse_buttons` / `mouse_clicks`).
    fn frame(held: bool, latch: bool) -> serde_json::Value {
        serde_json::json!({
            "mouse_buttons": {"left": false, "right": held},
            "mouse_clicks": {"left": false, "right": latch},
        })
    }

    /// Feed a register trace through the latch-aligned law and return
    /// the aligned fire level per record.
    fn aligned(trace: &[(bool, bool)]) -> Vec<bool> {
        let mut prev_latch = (false, false);
        let mut out = Vec::new();
        for &(h, l) in trace {
            let v = frame(h, l);
            let (held, latch) = raw_input_mc2(Some(&v));
            out.push(align_cmd_mc2(held, latch, prev_latch).fire_right);
            prev_latch = latch;
        }
        out
    }

    /// The pre-2026-08-04 mapping: held ∥ latch, merged (the phase then
    /// came from a uniform `--input-delay` ring, which cannot model a
    /// per-press split).
    fn legacy(trace: &[(bool, bool)]) -> Vec<bool> {
        trace
            .iter()
            .map(|&(h, l)| sample_cmd_mc2(Some(&frame(h, l))).fire_right)
            .collect()
    }

    fn edges(level: &[bool]) -> Vec<usize> {
        (1..level.len())
            .filter(|&i| level[i] && !level[i - 1])
            .collect()
    }

    /// A press the recorder caught with the latch STILL UP was not yet
    /// consumed at snapshot time: retail polls it on the NEXT frame, so
    /// the aligned edge is one record later than the raw held edge.
    /// (The legacy merge puts it on the raw record — this is the whole
    /// ±1 cast-phase split, and the assert fails under it.)
    #[test]
    fn mc2_pending_latch_defers_the_cast_one_record() {
        // r:      0        1       2       3       4
        let trace = [
            (false, false),
            (true, true),
            (true, false),
            (true, false),
            (false, false),
        ];
        assert_eq!(edges(&aligned(&trace)), vec![2]);
        assert_eq!(edges(&legacy(&trace)), vec![1]);
    }

    /// A press already CONSUMED by the snapshot's own frame (latch down
    /// on the record where held first reads 1) casts on that record —
    /// here the two mappings agree, which is why no uniform delay can
    /// serve both cases.
    #[test]
    fn mc2_consumed_press_casts_on_its_own_record() {
        let trace = [(false, false), (true, false), (true, false), (false, false)];
        assert_eq!(edges(&aligned(&trace)), vec![1]);
        assert_eq!(edges(&legacy(&trace)), vec![1]);
    }

    /// One cast per physical click, however long the hold — the level
    /// stays up for the whole hold (the repeat family reads it) but
    /// rises exactly once.
    #[test]
    fn mc2_long_hold_is_one_aligned_edge() {
        let mut trace = vec![(false, false), (true, true)];
        trace.extend(std::iter::repeat_n((true, false), 20));
        trace.push((false, false));
        let a = aligned(&trace);
        assert_eq!(edges(&a), vec![2]);
        assert!(a[2..22].iter().all(|&v| v), "the hold must stay armed");
    }

    /// A click shorter than the recorder's poll (latch caught, held
    /// already released at the next record) still casts — once, on the
    /// frame that consumed the latch. The legacy merge fires it a
    /// record early instead.
    #[test]
    fn mc2_sub_poll_click_casts_once_on_the_consuming_record() {
        let trace = [(false, false), (true, true), (false, false), (false, false)];
        assert_eq!(edges(&aligned(&trace)), vec![2]);
        assert_eq!(edges(&legacy(&trace)), vec![1]);
    }

    /// The cursor-AT-PRESS lane is decoded from the recorded frame, and
    /// it is the LIVE cursor that must never be mistaken for it.
    #[test]
    fn mc2_press_position_decodes_from_the_recorded_frame() {
        let v = serde_json::json!({
            "mouse": {"x": 396, "y": 185},
            "mouse_press_pos": {"x": 392, "y": 185},
        });
        assert_eq!(press_pos_mc2(Some(&v)), Some((392, 185)));
        assert_eq!(press_pos_mc2(Some(&serde_json::json!({}))), None);
        assert_eq!(press_pos_mc2(None), None);
    }

    /// The `MGC_PRESS_EDGE` fold turns a press-position CHANGE into a
    /// cast on the record that carries it, attributed to the button the
    /// record shows down — the sub-poll press the latch lane cannot
    /// see. Neutering the fold (`moved = false`) leaves the aligned
    /// command untouched, which is the default the corpus measurement
    /// picked (see [`press_edge_mc2`]).
    #[test]
    fn mc2_press_move_can_carry_a_cast_the_latch_missed() {
        let quiet = PlayerCommand::default();
        let folded = press_edge_mc2(quiet, (false, true), (false, false), true);
        assert!(folded.fire_right && !folded.fire_left);
        // Same record, no press-position move: nothing is manufactured.
        let same = press_edge_mc2(quiet, (false, true), (false, false), false);
        assert!(!same.fire_right && !same.fire_left);
        // Fully released at the snapshot: retail's registers cannot
        // attribute the press, so both hands take it.
        let both = press_edge_mc2(quiet, (false, false), (false, false), true);
        assert!(both.fire_left && both.fire_right);
    }

    fn ring_player(
        mb: u32,
        menu: u8,
        pending: u8,
        cursor: u8,
    ) -> mgc_formats::mgcr::RetailPlayerMc2 {
        mgc_formats::mgcr::RetailPlayerMc2 {
            move_bits: mb,
            menu_state: menu,
            hand_pending: pending,
            ring_cursor: cursor,
            ..Default::default()
        }
    }

    /// ROUND 104 — the 0x40 lane reads the RECORDED command word, not
    /// the press latches. PI:806/836/880-84 are how the input layer
    /// RAISES the bit; the recording carries the result on the same
    /// `move_bits` the two hand bits ride, so reconstructing it was
    /// asking a question `sub_5F380`'s bare `testb $0x40` never asks —
    /// and it is why the lane read "corpus-unreachable" while mc2l22
    /// t=63318 was tripping it.
    #[test]
    fn mc2_ring_cast_bit_rides_the_recorded_command_word() {
        // The pane coordinates the sender uses, with the bit set.
        assert_eq!(ring_cast_mc2(&ring_player(0x40, 5, 0, 9)), Some(9));
        assert_eq!(ring_cast_mc2(&ring_player(0x40, 8, 0, 0)), Some(0));
        // mc2l22 t=63318 verbatim: the bit ALONE, both hand bits clear.
        assert_eq!(ring_cast_mc2(&ring_player(64, 5, 0, 20)), Some(20));
        // No bit, no cast — however inviting the pane state looks.
        assert_eq!(ring_cast_mc2(&ring_player(0, 5, 0, 9)), None);
        assert_eq!(ring_cast_mc2(&ring_player(0x30, 5, 0, 9)), None);
        // The cursor's three padding cells are not spells.
        assert_eq!(ring_cast_mc2(&ring_player(0x40, 5, 0, 27)), None);
    }

    /// ⭐ THE TEAR GATE'S CADENCE LAW (round 149, dig w149j).
    /// `UpdateEntities_57730` (NETHERW.EXE file 0x7BF30, VA 0x57730)
    /// bumps `byte_0x3E_62` ONLY inside the arm that ran a handler:
    ///
    /// ```text
    ///   7c26d: mov  0x4(%eax),%cx      ; row.word_4
    ///   7c274: cmp  %cx,%dx            ; == action?
    ///   7c277: je   7c284              ;   no  -> sub_7C710, NO BUMP
    ///   7c284: cmpl $0x0,0xa(%eax)     ; row.dword_10
    ///   7c288: je   7c299              ;   0   -> NO BUMP
    ///   7c28a: call *0x6(%eax)         ; row.address_6 (the handler)
    ///   7c28e: mov  0x3e(%ebx),%ch
    ///   7c291: inc  %ch
    ///   7c296: mov  %ch,0x3e(%ebx)     ; THE BUMP
    /// ```
    ///
    /// The shipped tables give the null rows verbatim — class 5 0xEA
    /// `{0x002A5C30, 0x00EA, 0x00000000, 0x00000000}` (EF:1479) and
    /// class 10 0x52/0x54 `{0x002A5C44, 0x0052/0x0054, 0, 0}`
    /// (EF:1686/1688) — so those entities HOLD their phase byte for
    /// life. Measured on the mc2l24 recording: (5,27) act 0xEA is +0
    /// on 1,591,335 of 1,591,470 consecutive-tick pairs; (10,75) act
    /// 0x52 is +0 on 28,006 of 28,006; (10,77) act 0x54 on 49,875 of
    /// 49,883. Calling that a capture tear hid 82% of mc2l24's
    /// exclusions on ONE species.
    #[test]
    fn mc2_a_null_dispatch_row_holds_its_phase_byte_and_is_not_torn() {
        let hold = |class: u8, action: u8| {
            let mut a = RetailEntMc2 { class3f: class, model40: 27, action45: action, phase3e: 77, ..Default::default() };
            a.model40 = if class == 5 { 27 } else { 77 };
            let b = a;
            slot_is_torn(&a, &b)
        };
        // The m27 tier-2 spline segment and both fire-sphere satellites.
        assert!(!hold(5, 0xEA));
        assert!(!hold(10, 0x52));
        assert!(!hold(10, 0x54));
        // The m27 BRANCH is a null row too, but `sub_29A90` (EF:19827)
        // bumps it out of band every tick, so a HELD branch IS a tear.
        assert!(hold(5, 0xE9));
        // A dispatched row that held is a tear, as before.
        assert!(hold(5, 0xD9));
        assert!(hold(10, 0x00));
    }

    /// The same pair under `MGC_TEAR_LEGACY=1` is torn — the kill
    /// switch's POSITIVE CONTROL, asserted through the same function
    /// the gate calls (the env read is a `OnceLock`, so this test
    /// drives `mc2_no_bump_action` directly rather than racing it).
    #[test]
    fn mc2_the_legacy_tear_test_is_the_bare_plus_one() {
        // The law's whole content: which (class, action) rows are
        // no-bump. Legacy answers "none of them".
        assert!(mc2_no_bump_action(5, 0xEA));
        assert!(mc2_no_bump_action(10, 0x52));
        assert!(mc2_no_bump_action(10, 0x54));
        assert!(!mc2_no_bump_action(5, 0xE9));
        assert!(!mc2_no_bump_action(9, 0x0E));
        // Class 5's dispatched span either side of the null block.
        assert!(!mc2_no_bump_action(5, 0x27));
        assert!(mc2_no_bump_action(5, 0x28));
        assert!(mc2_no_bump_action(5, 0x47));
        assert!(!mc2_no_bump_action(5, 0x48));
    }

    /// ⭐⭐⭐ A REBIRTH'S PHASE BYTE IS ITS OWN SLOT INDEX — the EXACT
    /// witness, where the life-rose/out-of-range pair below are only
    /// heuristics. `NewEvent_4A050` (shipped `NETHERW.EXE` file
    /// 0x6E850) divides `record − pool_base` by the 0xA8 stride
    /// (`6e91f: be a8 00 00 00` / `6e927: f7 fe`) and stores the low
    /// byte of the quotient: `6e976: 88 43 3e  mov [ebx+0x3e],al`.
    /// ⛔ It is NOT `array_0x10[model]++` — that is the class-5
    /// creature ctor's counter and governs none of these families.
    /// Cross-tabbed over the whole of mc2l22, the slot-index test
    /// confirms 12,471 pairs the round-149 heuristics missed ((9,9)
    /// 7,903 · (9,13) 2,762 · (9,0) 1,711 · (10,11) 94) and takes the
    /// gate's leftover from 12,472 slot-exclusions to ONE.
    /// ⚠ REVERSION PROBE: `MGC_TEAR_NO_SLOT_SEED=1` disarms the clause
    /// and the first two asserts below fail.
    #[test]
    fn mc2_a_reborn_slot_carries_its_own_slot_index_as_its_phase() {
        let spark = |ph: u8, life: i32| RetailEntMc2 {
            class3f: 9,
            model40: 9,
            action45: 0x0E,
            phase3e: ph,
            life,
            ..Default::default()
        };
        // mc2l22's residue shape: |d| <= 2 AND life still falling —
        // invisible to both round-149 clauses — but the new phase IS
        // the slot's own index, so the record was re-minted here.
        // (slot 934 seeds 934 & 0xFF = 166; d = -1 is inside the
        // tear-reachable set, so ONLY this clause can acquit it.)
        assert!(!slot_is_torn_at(934, &spark(167, -2), &spark(166, -3)));
        // …and one past it, for a newborn minted above the walk
        // cursor that then took its own dispatch (mc2l16 t=7903).
        assert!(!slot_is_torn_at(934, &spark(168, -2), &spark(167, -3)));
        // A phase that is NOT this slot's seed stays a tear.
        assert!(slot_is_torn_at(934, &spark(169, -2), &spark(168, -3)));
        // …and the clause is per-SLOT: the same bytes at slot 935 are
        // still a tear, which is what makes it an exact witness rather
        // than another blanket exclusion.
        assert!(slot_is_torn_at(935, &spark(167, -2), &spark(166, -3)));
    }

    /// ⭐ A REBIRTH IS NOT A TEAR. Retail's `life_0x8` counts DOWN in
    /// the handler (class 9 action 0x0E is `sub_67410`//248410, four
    /// instructions: `life--; if (old < 0) retire`), so a life that
    /// ROSE across the pair is a slot that was freed and re-allocated
    /// inside the interval and re-seeded from the per-model spawn
    /// ordinal `D41A0_0.array_0x10[model]++` (EF:20780). Measured on
    /// the whole mc2l24 recording for the (9,9) spark: EVERY one of
    /// the 278,523 `+1` pairs has life FALLING, and 179,457 of the
    /// 180,248 non-`+1` pairs have life RISING. Excluding them cost
    /// 169,271 unshadowed slot-boundaries on mc2l24 alone.
    #[test]
    fn mc2_a_reborn_slot_is_a_new_instance_not_a_capture_tear() {
        let spark = |ph: u8, life: i32| RetailEntMc2 {
            class3f: 9,
            model40: 9,
            action45: 0x0E,
            phase3e: ph,
            life,
            ..Default::default()
        };
        // The mc2l24 slot-296 receipt: phase 41 → 40 while life goes
        // −2 → −1. A countdown cannot rise; the slot was re-allocated.
        assert!(!slot_is_torn(&spark(41, -2), &spark(40, -1)));
        // The same phase move with life still FALLING is a real tear.
        assert!(slot_is_torn(&spark(41, -2), &spark(40, -3)));
        // The ordinary dispatched pair is never torn.
        assert!(!slot_is_torn(&spark(41, 5), &spark(42, 4)));
        // A delta no tear can reach (mc2l24's (10,0) −10 family) is a
        // re-seed even with life falling.
        let mine = |ph: u8, life: i32| RetailEntMc2 {
            class3f: 10,
            model40: 0,
            action45: 0x00,
            phase3e: ph,
            life,
            ..Default::default()
        };
        assert!(!slot_is_torn(&mine(60, 9), &mine(50, 8)));
        // …but −1 / 0 / +2 stay inside the tear-reachable set.
        assert!(slot_is_torn(&mine(60, 9), &mine(59, 8)));
        assert!(slot_is_torn(&mine(60, 9), &mine(62, 8)));
    }
}
