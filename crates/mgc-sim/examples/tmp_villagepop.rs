//! MC1 village population-dynamics probe: boot level-000, park the
//! player far from the beach village, run the autonomous sim for a long
//! horizon, and sample population + spatial spread + deaths so we can
//! compare against the retail observation (retail: stable/declining,
//! tightly clustered; ours: suspected explosion + dispersal).
//! Usage: cargo run -p mgc-sim --example tmp_villagepop [level] [ticks]
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use std::collections::HashSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let level: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let horizon: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40000);

    let root = std::path::Path::new("baked");
    let path = root.join(format!("mc1/level-{level:03}.mgcl"));
    let pkg = mgc_formats::mgcl::read(std::fs::File::open(&path).unwrap()).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc1);

    // Park the player in a far corner, high up, so the wizard never
    // aggravates the village (militia +528 wanted-timer stays 0).
    let pose = PlayerPose::from_tiles(250.0, 40.0, 250.0, 0.0, 0.0, 0.0);
    let idle = PlayerCommand::default();

    // Death accounting by disappearing (slot, generation) identity.
    let mut prev: HashSet<(u16, u32)> = HashSet::new();
    let mut deaths_by_model: std::collections::BTreeMap<u8, u32> =
        std::collections::BTreeMap::new();
    let mut prev_model: std::collections::HashMap<(u16, u32), u8> =
        std::collections::HashMap::new();

    println!(
        "level {level}  horizon {horizon}  (villager models: 4=militia 12=settler 13=villager 14=migrant; houses=class10 m45)"
    );
    println!(
        "  tick | houses | m4  m12  m13  m14 | totV | meanD maxD (tiles from nearest house) | deaths m4/m12/m13/m14"
    );

    for t in 0..horizon {
        w.tick(pose, idle);

        let poses = w.live_poses();
        // Death diff.
        let cur: HashSet<(u16, u32)> = poses.iter().map(|p| (p.slot, p.generation)).collect();
        for gone in prev.difference(&cur) {
            if let Some(&m) = prev_model.get(gone) {
                if matches!(m, 4 | 12 | 13 | 14) {
                    *deaths_by_model.entry(m).or_insert(0) += 1;
                }
            }
        }
        prev = cur;
        prev_model = poses
            .iter()
            .map(|p| ((p.slot, p.generation), p.model))
            .collect();

        if t % 2000 != 1999 {
            continue;
        }

        let houses: Vec<(f32, f32)> = poses
            .iter()
            .filter(|p| p.class == 10 && p.model == 45)
            .map(|p| (p.x, p.z))
            .collect();
        let mut m = [0u32; 4]; // 4,12,13,14
        let mut dists: Vec<f32> = Vec::new();
        for p in &poses {
            let idx = match p.model {
                4 if p.class == 5 => 0,
                12 if p.class == 5 => 1,
                13 if p.class == 5 => 2,
                14 if p.class == 5 => 3,
                _ => continue,
            };
            m[idx] += 1;
            // distance to nearest house (torus 256)
            let mut best = f32::INFINITY;
            for &(hx, hz) in &houses {
                let dx = tor(p.x - hx);
                let dz = tor(p.z - hz);
                best = best.min((dx * dx + dz * dz).sqrt());
            }
            if best.is_finite() {
                dists.push(best);
            }
        }
        let totv: u32 = m.iter().sum();
        let (mean_d, max_d) = if dists.is_empty() {
            (0.0, 0.0)
        } else {
            let s: f32 = dists.iter().sum();
            (
                s / dists.len() as f32,
                dists.iter().cloned().fold(0.0, f32::max),
            )
        };
        let d = &deaths_by_model;
        println!(
            "  {:5} | {:5}  | {:3} {:4} {:4} {:4} | {:4} | {:5.1} {:5.1} | {}/{}/{}/{}",
            t + 1,
            houses.len(),
            m[0],
            m[1],
            m[2],
            m[3],
            totv,
            mean_d,
            max_d,
            d.get(&4).copied().unwrap_or(0),
            d.get(&12).copied().unwrap_or(0),
            d.get(&13).copied().unwrap_or(0),
            d.get(&14).copied().unwrap_or(0),
        );
    }
}

/// Torus wrap on the 256-tile map to the shortest signed delta.
fn tor(mut d: f32) -> f32 {
    while d > 128.0 {
        d -= 256.0;
    }
    while d < -128.0 {
        d += 256.0;
    }
    d
}
