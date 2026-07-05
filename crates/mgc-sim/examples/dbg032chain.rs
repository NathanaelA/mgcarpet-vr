//! Level-032 trigger-chain probe: drive the player through every live
//! proximity trigger repeatedly and log pool pressure, spawns, deaths
//! and trigger firings — distinguishing the two chain-stall hypotheses
//! (pool exhaustion vs kill-triggers misfiring on movement deaths).

use mgc_sim::features::{FeatureAssets, Planes};
use mgc_sim::world::{PlayerCommand, PlayerPose, World};
use std::collections::HashSet;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from("baked");
    let file = std::fs::File::open(root.join("mc1/level-032.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new(planes, &pkg.things.things, seed, assets);

    let stat = |w: &World| {
        let (free, ev) = w.debug_pool();
        let creatures = ev.iter().filter(|e| e.class == 5 && e.state != 120).count();
        let dead = ev
            .iter()
            .filter(|e| e.class == 5 && e.state != 120 && e.life < 0)
            .count();
        let triggers: Vec<_> = ev
            .iter()
            .filter(|e| e.class == 11)
            .map(|e| (e.slot, e.state, e.id24, e.tx, e.ty))
            .collect();
        let portals: Vec<_> = ev
            .iter()
            .filter(|e| e.class == 10 && e.state == 36)
            .map(|e| (e.tx, e.ty))
            .collect();
        (free, creatures, dead, triggers, portals)
    };

    let (free, creatures, dead, triggers, portals) = stat(&w);
    println!("== init: free {free}, creatures {creatures} ({dead} dead), portals {portals:?}");
    for t in &triggers {
        println!("   trigger slot {} state {} fires-dis {} at ({},{})", t.0, t.1, t.2, t.3, t.4);
    }

    let mut visited: HashSet<(usize, u16, u8, u8)> = HashSet::new();
    for round in 0..40 {
        // Next unvisited proximity trigger (states 0-12); slots get
        // reused, so identity includes the fired disposition + tile.
        let (_, ev) = w.debug_pool();
        let next = ev
            .iter()
            .filter(|e| e.class == 11 && e.state <= 12)
            .map(|e| (e.slot, e.tx, e.ty, e.id24))
            .find(|&(slot, tx, ty, dis)| !visited.contains(&(slot, dis, tx, ty)));
        let Some((slot, tx, ty, dis)) = next else {
            println!("== round {round}: no unvisited proximity triggers left");
            break;
        };
        visited.insert((slot, dis, tx, ty));
        let (x, z) = (tx as f32 + 0.5, ty as f32 + 0.5);
        let before = stat(&w);
        for _ in 0..24 {
            let alt = w.ground_height_tiles(x, z) + 0.5;
            w.tick(PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0), PlayerCommand::default());
            w.take_teleport();
        }
        let after = stat(&w);
        println!(
            "== visit trigger slot {slot} (dis {dis}) at ({tx},{ty}): free {} -> {}, creatures {} -> {} (dead {} -> {}), portals {:?} -> {:?}",
            before.0, after.0, before.1, after.1, before.2, after.2, before.4, after.4
        );
        for t in &after.3 {
            if !before.3.contains(t) {
                println!("   NEW trigger slot {} state {} fires-dis {} at ({},{})", t.0, t.1, t.2, t.3, t.4);
            }
        }
        for t in &before.3 {
            if !after.3.contains(t) {
                println!("   GONE trigger slot {} state {} fires-dis {} at ({},{})", t.0, t.1, t.2, t.3, t.4);
            }
        }
    }
    let (free, creatures, dead, triggers, portals) = stat(&w);
    println!("== final: free {free}, creatures {creatures} ({dead} dead), portals {portals:?}");
    for t in &triggers {
        println!("   remaining trigger slot {} state {} fires-dis {} at ({},{})", t.0, t.1, t.2, t.3, t.4);
    }
}
