//! Phase-4.3 misfit-sweep RUNTIME probe: boot every baked MC2 level,
//! release every disposition, tick the world with an idle far-away
//! player, and report the union of the misfit ledgers plus any level
//! that panics. The static THING census is `mc2census`; this catches
//! the runtime-only paths (dis-fired spawns, creature/effect children,
//! impact fall-throughs).
use mgc_formats::LevelPackage;
use mgc_sim::ids::GameId;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use std::collections::BTreeMap;

fn main() {
    // Usage: mc2sweep [ticks] [--combat]
    // --combat: a longer, hotter profile — the player hovers mid-map
    // firing native fireballs, so creature aggro/attack/impact paths
    // run too (default profile parks the player far away, idle).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let combat = args.iter().any(|a| a == "--combat");
    let ticks: u32 = args
        .iter()
        .find_map(|s| s.parse().ok())
        .unwrap_or(if combat { 512 } else { 64 });
    let root = std::path::Path::new("baked");
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.sprites.as_ref() {
        Some((sidx, _)) => {
            let dims: Vec<(u16, u16)> = sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
            assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims))
        }
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).unwrap(),
        None => assets,
    };

    let mut names: Vec<_> = std::fs::read_dir(root.join("mc2"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    names.sort();

    let mut tally: BTreeMap<(u16, u16), (u32, std::collections::BTreeSet<String>)> =
        BTreeMap::new();
    let mut panics: Vec<(String, String)> = Vec::new();
    for path in names {
        let lvl = path.file_stem().unwrap().to_string_lossy().into_owned();
        let assets = assets.clone();
        let result = std::panic::catch_unwind(move || {
            let file = std::fs::File::open(&path).unwrap();
            let pkg: LevelPackage = mgc_formats::mgcl::read(file).unwrap();
            let terrain = pkg.terrain.as_ref().unwrap();
            let planes = Planes {
                height: terrain.height.clone(),
                tile_type: terrain.tile_type.clone(),
                shading: terrain.shading.clone().unwrap(),
                angle: terrain.angle.clone().unwrap(),
                ceiling: terrain.ceiling.clone().unwrap_or_default(),
            };
            let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
            let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
            w.set_mc2_night_shade(true);
            // The rival column (4.3b): wire the level's wizards like
            // the app does, so the sweep exercises the AI brains.
            let mut cfgs: [Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8] = Default::default();
            let mut n = 1u16;
            if let (Some(wz), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) {
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
            for dis in 1..=64 {
                w.debug_fire_disposition(dis);
            }
            if combat {
                // Hover mid-map and fire in re-clicked bursts (edge
                // every other tick) while slowly spinning, so acquire/
                // impact/corpse paths get exercised in every quadrant.
                let pose =
                    |t: u32| PlayerPose::from_tiles(32.0, 16.0, 32.0, (t as f32) * 0.01, -0.2, 0.0);
                for t in 0..ticks {
                    let cmd = PlayerCommand {
                        fire_left: t % 2 == 0,
                        ..Default::default()
                    };
                    w.tick(pose(t), cmd);
                }
            } else {
                let idle = PlayerCommand::default();
                let pose = PlayerPose::from_tiles(2.0, 20.0, 2.0, 0.0, 0.0, 0.0);
                for _ in 0..ticks {
                    w.tick(pose, idle);
                }
            }
            w.misfits().to_vec()
        });
        match result {
            Ok(misfits) => {
                for (c, m, n) in misfits {
                    let e = tally.entry((c, m)).or_default();
                    e.0 += n;
                    e.1.insert(lvl.clone());
                }
            }
            Err(e) => {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "<non-string panic>".into());
                panics.push((lvl, msg));
            }
        }
    }
    println!(
        "runtime misfit union (class, model): levels-with / examples\n\
         scope: {} ticks/level, dispositions 1..=64 fired blind, rival\n\
         brains wired, NO stage engine, {} — an empty union means none\n\
         SURFACED under this profile, not that every path is covered",
        ticks,
        if combat {
            "combat profile (mid-map, firing)"
        } else {
            "idle far-away player (try --combat)"
        }
    );
    if tally.is_empty() {
        println!("  (none)");
    }
    for ((c, m), (_, lv)) in &tally {
        let ex: Vec<_> = lv.iter().take(5).cloned().collect();
        println!(
            "({c:2},{m:3})  {:<3} levels   e.g. {}",
            lv.len(),
            ex.join(", ")
        );
    }
    println!("\npanicking levels: {}", panics.len());
    for (lvl, msg) in &panics {
        println!("  {lvl}: {}", msg.lines().next().unwrap_or(""));
    }
}
