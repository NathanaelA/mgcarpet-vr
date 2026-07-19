//! Scratch (playtest-cave): why don't level-003's corridor doors
//! (pillar open/close) trigger? Enumerate the (14,2)/(10,63)/(10,64)
//! records + switches, then fly into a switch box and watch.
use mgc_sim::ids::GameId;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};

fn main() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let f = std::fs::File::open(root.join("mc2/level-003.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(f).unwrap();
    println!("== authored records of interest ==");
    for t in &pkg.things.things {
        let key = (t.class, t.model);
        if matches!(key, (14, 2) | (10, 63) | (10, 64)) || t.class == 11 {
            println!(
                "({},{}) at tile ({}, {}) dis_id {} swi_id {} swi_sz {} par1 {} par2 {} par3 {:?}",
                t.class,
                t.model,
                t.x >> 8,
                t.y >> 8,
                t.dis_id as i16,
                t.swi_id,
                t.swi_sz,
                t.parent,
                t.child,
                t.par3,
            );
        }
    }

    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-cave")).unwrap();
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    assets = assets
        .with_spells(bundle.spells.as_deref().unwrap())
        .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    w.set_mc2_night_shade(true);
    if let Some(st) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = st
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        w.set_mc2_stages(&rows);
    }

    println!("\n== live pool after init ==");
    for e in w.debug_pool().1 {
        if (e.class == 14 && e.model == 2) || e.class == 11 {
            println!(
                "pool ({},{}) at tile ({}, {}) life {} state {}",
                e.class, e.model, e.tx, e.ty, e.life, e.state
            );
        }
    }

    // Realistic sequence: far away (door closes at load), approach
    // (switch 34 opens it), leave (the (11,1) leave-switch recloses).
    let idle = PlayerCommand::default();
    let door = |w: &World| {
        let pl = w.planes();
        let c = w.ceiling_plane();
        let t = 43usize * 256 + 182;
        (pl.height[t], c[t], pl.angle[t] & 8)
    };
    let pillar_life = |w: &World| {
        w.debug_pool()
            .1
            .iter()
            .find(|e| e.class == 14 && e.model == 2 && e.tx == 182 && e.ty == 43)
            .map(|e| e.life)
    };
    let park = |w: &mut World, x: f32, y: f32, n: usize| {
        for _ in 0..n {
            let alt = w.ground_height_tiles(x, y) + 2.0;
            w.tick(PlayerPose::from_tiles(x, alt, y, 0.0, 0.0, 0.0), idle);
        }
    };
    println!("door at init: {:?} pillar {:?}", door(&w), pillar_life(&w));
    park(&mut w, 60.0, 60.0, 120); // far away
    println!(
        "after 120 far ticks: {:?} pillar {:?}",
        door(&w),
        pillar_life(&w)
    );
    park(&mut w, 182.5, 42.5, 60); // in the switch box
    println!(
        "after approach: {:?} pillar {:?}",
        door(&w),
        pillar_life(&w)
    );
    park(&mut w, 60.0, 60.0, 120); // leave
    println!("after leaving: {:?} pillar {:?}", door(&w), pillar_life(&w));
}
