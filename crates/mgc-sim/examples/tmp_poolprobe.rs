//! Scratch (playtest-cave round 2): what floods the pool on level 003?
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use std::collections::HashMap;

fn main() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let f = std::fs::File::open(root.join("mc2/level-003.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(f).unwrap();
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
    let idle = PlayerCommand::default();
    for tick in 0..4000usize {
        // MOVING player: orbit the mid-map so position AND yaw sweep
        // (crosses switch boxes, sweeps the drip search window).
        let a = tick as f32 * 0.01;
        let x = 128.0 + 60.0 * a.cos();
        let y = 128.0 + 60.0 * a.sin();
        let alt = w.ground_height_tiles(x, y) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, y, a + 1.57, 0.0, 0.0), idle);
        let dropped = w.take_pool_exhausted();
        if dropped > 0 {
            println!("t{tick}: POOL EXHAUSTED, {dropped} dropped");
        }
        if (18..=28).contains(&tick) {
            let pool = w.debug_pool();
            let mut tally: HashMap<(u8, u8), usize> = HashMap::new();
            for e in &pool.1 {
                *tally.entry((e.class, e.model)).or_default() += 1;
            }
            let mut v: Vec<_> = tally.into_iter().collect();
            v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            let live: usize = v.iter().map(|&(_, n)| n).sum();
            print!("  t{tick}: live {live} top:");
            for ((c, m), n) in v.into_iter().take(8) {
                print!(" ({c},{m})x{n}");
            }
            println!();
        }
        if tick % 250 == 249 {
            let pool = w.debug_pool();
            let mut tally: HashMap<(u8, u8), usize> = HashMap::new();
            for e in &pool.1 {
                *tally.entry((e.class, e.model)).or_default() += 1;
            }
            let mut v: Vec<_> = tally.into_iter().collect();
            v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            let live: usize = v.iter().map(|&(_, n)| n).sum();
            print!("t{}: live {} top:", tick + 1, live);
            for ((c, m), n) in v.into_iter().take(8) {
                print!(" ({c},{m})x{n}");
            }
            println!();
        }
    }
}
