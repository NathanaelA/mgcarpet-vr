//! Headless rival smoke probe: real baked level, 6000 ticks.
use std::path::Path;

fn main() {
    let lvl = std::env::args().nth(1).unwrap_or("002".into());
    let f = std::fs::File::open(format!("baked/mc1/level-{lvl}.mgcl")).unwrap();
    let package = mgc_formats::mgcl::read(std::io::BufReader::new(f)).unwrap();
    let dir = Path::new("baked/assets/mc1-temperate");
    let bundle = mgc_formats::bundle::Bundle::load(dir).unwrap();
    let terrain = package.terrain.as_ref().unwrap();
    let assets = mgc_sim::features::FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let planes = mgc_sim::features::Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
    };
    let seed = package.gen_params.as_ref().unwrap().seed;
    let mut w = mgc_sim::world::World::new(planes, &package.things.things, seed, assets);
    // wizards.json -> configs
    let wiz = package.wizards.as_ref().unwrap();
    let count = wiz.player_count.unwrap();
    let mut cfgs: [Option<mgc_sim::rivals::RivalConfig>; 8] = Default::default();
    for (slot, c) in wiz.wizards.iter().enumerate().take(8).skip(1) {
        let mut book = [false; 24];
        let mut allowed = [false; 24];
        let am = c.allowed_spells.as_ref().unwrap();
        for s in 0..24 {
            allowed[s] = am[s] != 0;
            book[s] = allowed[s] && c.starting_spells[s] != 0;
        }
        cfgs[slot] = Some(mgc_sim::rivals::RivalConfig {
            aggression: c.aggression as u8,
            accuracy: c.accuracy.unwrap() as u8,
            tempo: c.tempo.unwrap() as u8,
            castle_level: c.castle_level.unwrap(),
            book,
            allowed,
        });
    }
    w.set_wizards(&cfgs, count);
    println!("players={count} rivals={}", w.rival_views().len());
    for r in w.rival_views() {
        println!("  {} slot {} at ({:.0},{:.0})", r.name, r.slot, r.x, r.z);
    }
    // Park the player near the level start.
    let start = package
        .things
        .things
        .iter()
        .find(|t| t.class == 3 && t.model == 4)
        .unwrap();
    let pose = mgc_sim::world::PlayerPose::level(
        ((start.x as u32) << 8) as u16 + 128,
        ((start.y as u32) << 8) as u16 + 128,
        3000,
        0,
    );
    for t in 0..6000u32 {
        w.tick(pose, mgc_sim::world::PlayerCommand::default());
        for slot in w.take_rival_deaths() {
            println!("t{t}: rival slot {slot} died");
        }
        if t % 1000 == 999 {
            for r in w.rival_views() {
                let castle = if r.alive { "" } else { " (dead)" };
                println!(
                    "t{}: {} at ({:.0},{:.0}) mana {}/{} life {:.2}{}",
                    t + 1,
                    r.name,
                    r.x,
                    r.z,
                    r.mana,
                    r.mana_max,
                    r.life_frac,
                    castle
                );
            }
            let poses = w.live_poses();
            let castles = poses
                .iter()
                .filter(|p| p.class == 3 && p.model == 2)
                .count();
            let wiz = poses
                .iter()
                .filter(|p| p.class == 3 && p.model <= 1)
                .count();
            let projs = poses.iter().filter(|p| p.class == 9).count();
            println!("      castles={castles} wizard-billboards={wiz} projectiles={projs}");
        }
    }
}
