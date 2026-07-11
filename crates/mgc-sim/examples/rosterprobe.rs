//! Phase-4.3 smoke probe: boot the baked mc2 level-000, release every
//! disposition, tick with the player amid the population, and report
//! per-model class-5 counts, class-9 launches and the misfit ledger.
use mgc_formats::LevelPackage;
use mgc_sim::ids::GameId;
use mgc_sim::mc1::features::FeatureAssets;
use mgc_sim::mc1::features::Planes;
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};

fn main() {
    let root = std::path::Path::new("baked");
    let file = std::fs::File::open(root.join("mc2/level-000.mgcl")).unwrap();
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
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
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_mc2_night_shade(true);
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }
    let idle = PlayerCommand::default();
    let mut c9_seen = std::collections::BTreeMap::new();
    let mut sounds = std::collections::BTreeMap::new();
    // Park next to the first (5,19) firebug so the wave engages.
    let anchor = w
        .live_poses()
        .iter()
        .find(|p| p.class == 5 && p.model == 19)
        .map(|p| (p.x, p.z));
    println!("firebug anchor: {anchor:?}");
    for t in 0..900 {
        let (x, z) = anchor.map_or_else(
            || (118.0 + (t % 60) as f32 * 0.4, 210.0),
            |(ax, az)| (ax + 2.0, az),
        );
        let alt = w.ground_height_tiles(x, z) + 2.0;
        let pose = PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0);
        w.tick(pose, idle);
        for s in &w.take_audio(pose).events {
            *sounds.entry(s.id).or_insert(0u32) += 1;
        }
        for p in w.live_poses() {
            if p.class == 9 {
                *c9_seen.entry(p.model).or_insert(0u32) += 1;
            }
        }
    }
    let mut c5 = std::collections::BTreeMap::new();
    for p in w.live_poses() {
        if p.class == 5 {
            *c5.entry(p.model).or_insert(0u32) += 1;
        }
    }
    println!("live class-5 by model: {c5:?}");
    println!("class-9 pose-ticks by model: {c9_seen:?}");
    println!("misfits: {:?}", w.misfits());
    println!("sound ids heard: {sounds:?}");
}
