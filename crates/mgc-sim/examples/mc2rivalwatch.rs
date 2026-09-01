//! FRESH-START RIVAL WATCH — the free-play arm of the "inert rivals"
//! investigation (dig D3).
//!
//! Boots ONE baked MC2 level exactly the way `WorldInit::build`
//! (mgc-app) and `build_world_mc2` (mgc-conform) do — same bundle
//! variant law, same stages/stagevars/wizards wiring — parks an idle
//! human far away, free-runs N ticks, and reports what every rival
//! brain actually did: the `ai_state` histogram, the state-transition
//! log, distance travelled from the spawn marker, and the castle /
//! charge / cooldown lanes.
//!
//! There is NO recording and NO import here: this is precisely the
//! path a player takes when they start a level from the menu, which
//! is the arm the conformance harness never grades.
//!
//! ```text
//! cargo run --release -p mgc-sim --example mc2rivalwatch -- 22 3000
//! cargo run --release -p mgc-sim --example mc2rivalwatch -- 22 3000 --player 64,64
//! ```
use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use std::collections::BTreeMap;

fn scalar(ws: &mgc_sim::engine::world::conformance::WizShadowMc2, name: &str) -> i64 {
    ws.scalars
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(0, |(_, v)| *v)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let level: u32 = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(22);
    let ticks: u32 = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    // Where the human parks. Default = the far corner (the "player
    // never approaches" arm the observation describes).
    let (ptx, pty) = args
        .iter()
        .position(|a| a == "--player")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| {
            let (a, b) = s.split_once(',')?;
            Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
        })
        .unwrap_or((2.0, 2.0));
    let every: u32 = args
        .iter()
        .position(|a| a == "--every")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(250);

    let root = std::path::Path::new("baked");
    let lp = root.join("mc2").join(format!("level-{level:03}.mgcl"));
    let file = std::fs::File::open(&lp).unwrap_or_else(|e| panic!("{}: {e}", lp.display()));
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let header = pkg.header.as_ref();
    let variant = match header.map(|h| (h.map_type, h.gfx_type)) {
        Some((mgc_formats::MapType::Night, g)) if g & 2 != 0 => "mc2-night-fog",
        Some((mgc_formats::MapType::Night, _)) => "mc2-night",
        Some((mgc_formats::MapType::Cave, _)) => "mc2-cave",
        _ => "mc2-day",
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join(variant)).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    if let Some(dims) = bundle.mc2_extent_dims(&root.join("assets")) {
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp).unwrap();
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    w.set_mc2_night_shade(matches!(
        header.map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    ));
    w.set_mc2_doom_level(header.is_some_and(|h| h.gfx_type & 2 != 0));
    w.set_mc2_castle_purge_level(header.is_some_and(|h| h.gfx_type & 4 != 0));
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
    let mut cfgs: [Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8] = Default::default();
    let mut n = 1u16;
    if let (Some(wz), Some(h)) = (pkg.wizards.as_ref(), header) {
        n = h.number_of_players.clamp(1, 8) as u16;
        for (slot, cfg) in wz.wizards.iter().enumerate().take(8).skip(1) {
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
            cfgs[slot] = Some(mgc_sim::mc2::rivals::Mc2RivalConfig {
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
    }
    w.set_mc2_wizards(&cfgs, n);
    println!("level {level} variant {variant} players {n} ticks {ticks} player_tile ({ptx},{pty})");
    for (slot, c) in cfgs.iter().enumerate() {
        if let Some(c) = c {
            let known: Vec<usize> = (0..26).filter(|&s| c.start[s] && !c.blocked[s]).collect();
            println!(
                "cfg slot {slot}: agg {} per {} refl {} life {} castle_lvl {} spells {known:?}",
                c.aggression, c.perception, c.reflexes, c.life, c.castle_level
            );
        }
    }

    // Spawn positions, for the "did it ever leave the flag" measure.
    let mut home: BTreeMap<u8, (u16, u16)> = BTreeMap::new();
    let mut hist: BTreeMap<(u8, i64), u32> = BTreeMap::new();
    let mut last_state: BTreeMap<u8, i64> = BTreeMap::new();
    let mut maxd: BTreeMap<u8, f64> = BTreeMap::new();
    let pose = PlayerPose::from_tiles(ptx, 20.0, pty, 0.0, 0.0, 0.0);
    let idle = PlayerCommand::default();
    for t in 0..ticks {
        w.tick(pose, idle);
        for ws in w.wiz_shadow_mc2() {
            if ws.wiz == 0 {
                continue;
            }
            let st = scalar(&ws, "ai_state");
            *hist.entry((ws.wiz, st)).or_default() += 1;
            let lanes = w.port_ent_lanes_mc2(ws.ent, 0, false).unwrap_or_default();
            let get = |name: &str| {
                lanes
                    .iter()
                    .find(|(n, _)| *n == name)
                    .and_then(|(_, v)| *v)
                    .unwrap_or(0)
            };
            let (x, y) = (get("x") as u16, get("y") as u16);
            let h = *home.entry(ws.wiz).or_insert((x, y));
            let dx = (x as f64 - h.0 as f64) / 256.0;
            let dy = (y as f64 - h.1 as f64) / 256.0;
            let d = (dx * dx + dy * dy).sqrt();
            let e = maxd.entry(ws.wiz).or_insert(0.0);
            if d > *e {
                *e = d;
            }
            let prev = last_state.insert(ws.wiz, st);
            if prev.is_some_and(|p| p != st) {
                println!(
                    "t={t} wiz {} ent {} state {} -> {st}  pos ({:.1},{:.1}) d {d:.1} \
                     castle {} charge {} life {}",
                    ws.wiz,
                    ws.ent,
                    prev.unwrap(),
                    x as f64 / 256.0,
                    y as f64 / 256.0,
                    scalar(&ws, "castle_ent"),
                    scalar(&ws, "charge"),
                    get("life"),
                );
            }
        }
        if every > 0 && t % every == 0 {
            for ws in w.wiz_shadow_mc2() {
                if ws.wiz == 0 {
                    continue;
                }
                let lanes = w.port_ent_lanes_mc2(ws.ent, 0, false).unwrap_or_default();
                let get = |name: &str| {
                    lanes
                        .iter()
                        .find(|(n, _)| *n == name)
                        .and_then(|(_, v)| *v)
                        .unwrap_or(0)
                };
                let cds: Vec<(usize, i64)> = ws
                    .arrays
                    .iter()
                    .find(|(n, _)| *n == "cooldown")
                    .map(|(_, v)| {
                        v.iter()
                            .enumerate()
                            .filter(|&(_, &c)| c != 0)
                            .map(|(s, &c)| (s, c))
                            .collect()
                    })
                    .unwrap_or_default();
                let m3 = w.debug_mc2_rival_book(ws.wiz).map_or(0, |b| b[3]);
                let m3f26 = w
                    .port_ent_lanes_mc2(m3, 0, false)
                    .and_then(|l| l.iter().find(|(n, _)| *n == "f26").and_then(|(_, v)| *v))
                    .unwrap_or(-1);
                let tgt = get("target96") as u16;
                let tl = w.port_ent_lanes_mc2(tgt, 0, false).unwrap_or_default();
                let tget = |name: &str| {
                    tl.iter()
                        .find(|(n, _)| *n == name)
                        .and_then(|(_, v)| *v)
                        .unwrap_or(-999)
                };
                let tinfo = format!(
                    "T{tgt}[c{} m{} life{} act{} sv1{} x{} y{}]",
                    tget("class3f"),
                    tget("model40"),
                    tget("life"),
                    tget("action45"),
                    tget("sv1"),
                    tget("x"),
                    tget("y"),
                );
                let m1 = w.debug_mc2_rival_book(ws.wiz).map_or(0, |b| b[1]);
                let m1f26 = w
                    .port_ent_lanes_mc2(m1, 0, false)
                    .and_then(|l| l.iter().find(|(n, _)| *n == "f26").and_then(|(_, v)| *v))
                    .unwrap_or(-1);
                println!(
                    "[t={t}] wiz {} ent {} state {} target {} pos ({:.1},{:.1}) z {} \
                     life {}/{} castle {} charge {} burst {} pov {} spd {} cmd {} brake {} strafe {} m3 {m3}/f26 {m3f26} m1 {m1}/f26 {m1f26} sig {} {tinfo} cd {cds:?}",
                    ws.wiz,
                    ws.ent,
                    scalar(&ws, "ai_state"),
                    get("target96"),
                    get("x") as f64 / 256.0,
                    get("y") as f64 / 256.0,
                    get("z"),
                    get("life"),
                    get("max_life"),
                    scalar(&ws, "castle_ent"),
                    scalar(&ws, "charge"),
                    scalar(&ws, "burst"),
                    scalar(&ws, "poverty"),
                    get("speed"),
                    scalar(&ws, "cmd_speed"),
                    scalar(&ws, "brake"),
                    scalar(&ws, "strafe"),
                    get("f98"),
                );
            }
        }
    }
    println!("--- ai_state histogram (ticks in each state) ---");
    for (&(wiz, st), &c) in &hist {
        println!("wiz {wiz} state {st}: {c}");
    }
    println!("--- max distance from spawn (tiles) ---");
    for (wiz, d) in &maxd {
        println!("wiz {wiz}: {d:.1}");
    }
}
