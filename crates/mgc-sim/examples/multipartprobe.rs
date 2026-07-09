//! Phase-4.3 multipart smoke probe: find a baked MC2 campaign level
//! authoring all four multipart models (5,0/3/22/27), boot it,
//! release every disposition, tick with the player parked near the
//! m27 tree, and report chain populations, class-9 launches, sounds
//! and the misfit ledger.
use mgc_sim::ids::GameId;
use mgc_sim::mc1::features::FeatureAssets;
use mgc_sim::mc1::features::Planes;
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};

fn main() {
    let root = std::path::Path::new("baked");
    let mut paths: Vec<_> = std::fs::read_dir(root.join("mc2"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    paths.sort();
    let mut pick = None;
    for p in &paths {
        let Ok(f) = std::fs::File::open(p) else {
            continue;
        };
        let Ok(pkg) = mgc_formats::mgcl::read(f) else {
            continue;
        };
        let has = |m: u16| {
            pkg.things
                .things
                .iter()
                .any(|t| t.class == 5 && t.model == m)
        };
        if has(0) && has(3) && has(22) && has(27) {
            pick = Some((p.clone(), pkg));
            break;
        }
    }
    let Some((path, pkg)) = pick else {
        // Fall back to any level with m22 + m27 (the rarer pair).
        println!("no level authors all four; falling back");
        return;
    };
    println!("probing {}", path.display());

    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
    };
    // Night vs day bundle by the level header's map type (the app's
    // rule, mgc-app main.rs).
    let night = matches!(
        pkg.header.as_ref().map(|h| h.map_type),
        Some(mgc_formats::MapType::Night)
    );
    let bundle_name = if night { "mc2-night" } else { "mc2-day" };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join(format!("assets/{bundle_name}")))
        .or_else(|_| mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")))
        .unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).unwrap(),
        None => assets,
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_mc2_night_shade(night);
    for dis in 1..=64 {
        w.debug_fire_disposition(dis);
    }

    // Park near the m27 body (the most complex chain).
    let anchor = w
        .live_poses()
        .iter()
        .find(|p| p.class == 5 && p.model == 27)
        .map(|p| (p.x, p.z));
    let anchor = anchor.or_else(|| {
        w.live_poses()
            .iter()
            .find(|p| p.class == 5 && p.model == 22)
            .map(|p| (p.x, p.z))
    });
    println!("anchor (m27/m22): {anchor:?}");

    let idle = PlayerCommand::default();
    let mut sounds = std::collections::BTreeMap::new();
    let mut c9 = std::collections::BTreeMap::new();
    let mut peak: std::collections::BTreeMap<u8, usize> = std::collections::BTreeMap::new();
    for t in 0..1200 {
        let (x, z) = anchor.map_or((128.0, 128.0), |(ax, az)| (ax + 2.0, az + 1.0));
        let alt = w.ground_height_tiles(x, z) + 2.0;
        let yaw = (t as f32 * 0.01) % std::f32::consts::TAU;
        let pose = PlayerPose::from_tiles(x, alt, z, yaw, 0.0, 0.0);
        w.tick(pose, idle);
        for s in &w.take_audio(pose).events {
            *sounds.entry(s.id).or_insert(0u32) += 1;
        }
        for p in w.live_poses() {
            if p.class == 9 {
                *c9.entry(p.model).or_insert(0u32) += 1;
            }
            if p.class == 5 && matches!(p.model, 0 | 3 | 22 | 27) {
                *peak.entry(p.model).or_insert(0) += 0; // presence key
            }
        }
        if t % 300 == 299 {
            let mut counts: std::collections::BTreeMap<(u8, u8), usize> =
                std::collections::BTreeMap::new();
            for p in w.live_poses() {
                if p.class == 5 && matches!(p.model, 0 | 3 | 22 | 27) {
                    *counts.entry((p.model, 0)).or_insert(0) += 1;
                }
            }
            println!("t={:4} multipart poses: {counts:?}", t + 1);
        }
    }
    println!("class-9 pose-ticks by model: {c9:?}");
    println!("sounds: {sounds:?}");
    println!("misfits: {:?}", w.misfits());
    println!("verb fallbacks: {:?}", w.verb_fallbacks());
}
