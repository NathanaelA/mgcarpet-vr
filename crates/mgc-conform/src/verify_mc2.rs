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
//! RECORDING.md); casts reconstruct like MC1's, through the
//! `--input-delay` ring, with `fire = held || latch` (the latch is
//! set at the press edge, so a click shorter than one poll still
//! lands).

use crate::Args;
use crate::verify::{FieldDiff, PairDiff, Stats};
use mgc_formats::mgcr::{EntObsMc2, ObsMc2, Recording, RetailEntMc2, RetailMc2, decode_retail_mc2};
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::conformance::PinnedMc2;
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use std::collections::BTreeMap;

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
    let (mut world, pristine) = build_world_mc2(&args.baked, level)?;

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
    let mut cmd_ring: std::collections::VecDeque<PlayerCommand> =
        std::iter::repeat_n(PlayerCommand::default(), args.input_delay as usize + 1).collect();
    let mut stats = Stats::default();
    let mut printed_import = false;
    let mut boundary_seeded = false;
    while let Some(r) = rec.next_tick() {
        let tick = r?;
        let Some(state) = &tick.state else {
            prev = None;
            continue;
        };
        let st = decode_retail_mc2(state)?;
        let obs: ObsMc2 = match &tick.obs {
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("obs: {e}"))?,
            None => return Err(format!("t={}: no obs channel", tick.t)),
        };
        let sample = sample_cmd_mc2(tick.input.as_ref());
        if !boundary_seeded && tick.input.is_some() {
            boundary_seeded = true;
            // A button already held on the recording's FIRST frame has
            // no press edge inside the capture (retail latched it
            // before t=0), but the ring's default pre-fill reads
            // "released" and manufactures one — the t≈3 (9,17)-vs-
            // smoke misfire was the right button held across the
            // level boundary. Extend the first frame's held state
            // backward instead.
            for c in cmd_ring.iter_mut() {
                *c = sample;
            }
            prev_cmd = sample;
        }
        cmd_ring.push_back(sample);
        let cmd = cmd_ring.pop_front().unwrap_or_default();
        if let Some((pt, pst, pcmd)) = prev.take() {
            if args.start.is_some_and(|s| pt < s) {
                // Before the triage window — keep the pairing chain
                // and the input ring warm, execute nothing.
            } else if tick.t == pt + 1 {
                let announce = args.start.is_some();
                if announce {
                    eprintln!("pair {pt}");
                }
                stats.pairs += 1;
                if !capture_clean_mc2(&pst, &st) {
                    stats.torn += 1;
                } else {
                    let (pd, port, report) = exec_pair_mc2(
                        &mut world, &pristine, &pst, &st, &obs, pcmd, prev_cmd, pin_n1,
                    )
                    .map_err(|e| format!("t={pt}: {e}"))?;
                    let human_slot = report.human_slot;
                    if announce && let Some((got, want)) = report.stack_fallback {
                        eprintln!("  free-stack fallback: live {got} != scan {want}");
                    }
                    if !printed_import {
                        printed_import = true;
                        println!(
                            "   import: {} active entities, human slot {human_slot}",
                            obs.n_active
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
                        crate::verify::classify_pair(roster.as_ref(), &take, pt, &pd, &ctx)
                    });
                    // Pose-phase pass — see verify.rs (the MC1 twin).
                    if !args.no_pose_alt
                        && !pd.clean()
                        && let Some(tg) = tags.as_mut()
                    {
                        let (alt, _, _) = exec_pair_mc2(
                            &mut world, &pristine, &pst, &st, &obs, pcmd, prev_cmd, !pin_n1,
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
                            &mut world, &pristine, &pst, &st, &obs, pcmd, prev_cmd, pin_n1,
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
        }
        if let Some((_, _, c)) = &prev {
            prev_cmd = *c;
        }
        prev = Some((tick.t, st, cmd));
        if let Some(limit) = args.limit {
            if stats.pairs >= limit {
                break;
            }
        }
    }
    print!("{}", stats.render(args, roster.as_ref()));
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
        ..Default::default()
    }
}

/// Is the pair fixture-grade? See the module doc: step-1 dominance of
/// the per-entity phase byte across entities live (same class+model)
/// at both ends. Pairs with no live-in-both population (never happens
/// on real levels) fail closed.
pub(crate) fn capture_clean_mc2(pst: &RetailMc2, st: &RetailMc2) -> bool {
    let (mut d0, mut d1, mut d2) = (0u32, 0u32, 0u32);
    for slot in 1..pst.ents.len().min(st.ents.len()) {
        let (a, b) = (&pst.ents[slot], &st.ents[slot]);
        if a.class3f == 0 || a.class3f != b.class3f || a.model40 != b.model40 {
            continue;
        }
        match b.phase3e.wrapping_sub(a.phase3e) {
            0 => d0 += 1,
            1 => d1 += 1,
            2 => d2 += 1,
            _ => {}
        }
    }
    d1 > 0 && d1 >= d0 && d1 >= d2
}

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
    pst: &RetailMc2,
    st: &RetailMc2,
    obs: &ObsMc2,
    cmd: PlayerCommand,
    prev_cmd: PlayerCommand,
    pin_n1: bool,
) -> Result<
    (
        PairDiff,
        ObsMc2,
        mgc_sim::engine::world::conformance::ImportReport,
    ),
    String,
> {
    world.restore_planes(pristine);
    let report = world
        .retail_import_mc2(pst)
        .map_err(|e| format!("import: {e}"))?;
    world.set_prev_fire(prev_cmd.fire_left, prev_cmd.fire_right);
    let pose_src = if pin_n1 {
        &st.ents[report.human_slot as usize]
    } else {
        &pst.ents[report.human_slot as usize]
    };
    let pose = carpet_pose_mc2(pose_src);
    world.tick(pose, cmd);
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
    let torn = torn_slots(pst, st);
    let pd = compare_mc2_gated(obs, &port, report.human_slot, &torn);
    Ok((pd, port, report))
}

/// Slots live at both ends whose phase byte did NOT advance exactly
/// once — per-entity capture tear inside an accepted pair.
pub(crate) fn torn_slots(pst: &RetailMc2, st: &RetailMc2) -> std::collections::BTreeSet<u16> {
    let mut torn = std::collections::BTreeSet::new();
    for slot in 1..pst.ents.len().min(st.ents.len()) {
        let (a, b) = (&pst.ents[slot], &st.ents[slot]);
        if a.class3f == 0 || a.class3f != b.class3f || a.model40 != b.model40 {
            continue;
        }
        if b.phase3e.wrapping_sub(a.phase3e) != 1 {
            torn.insert(slot as u16);
        }
    }
    torn
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
) -> Result<(World, Planes), String> {
    let lp = baked.join("mc2").join(format!("level-{level:03}.mgcl"));
    let file = std::fs::File::open(&lp).map_err(|e| format!("{}: {e}", lp.display()))?;
    let pkg: mgc_formats::LevelPackage =
        mgc_formats::mgcl::read(file).map_err(|e| format!("{}: {e}", lp.display()))?;
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
    if let Some((sidx, _)) = bundle.sprites.as_ref() {
        let dims: Vec<(u16, u16)> = sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp)?;
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(
        planes,
        &pkg.things.things,
        seed,
        assets,
        mgc_sim::ids::GameId::Mc2,
    );
    w.set_placeholders(true);
    w.set_mc2_night_shade(matches!(
        header.map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    ));
    w.set_mc2_doom_level(header.is_some_and(|h| h.gfx_type & 2 != 0));
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
    let (wizards, player_count) = mc2_rival_configs(pkg.wizards.as_ref(), header);
    w.set_mc2_wizards(&wizards, player_count);
    let pristine = w.planes_clone();
    Ok((w, pristine))
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
        cmp_field!(out, s, "heading", re.heading, pe.heading);
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
