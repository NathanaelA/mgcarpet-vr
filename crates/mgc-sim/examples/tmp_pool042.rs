//! Pool-occupancy probe for MC1 level 042 (the kraken/genie brawl):
//! is the exhaustion a legitimately-full pool that RECOVERS, or a leak
//! that climbs to the 1999 cap and stays? Orbit the player to keep the
//! roaming mobs aggroed; log live occupancy, peak, drops, and the pool
//! composition at the busiest tick.
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let f = std::fs::File::open(root.join("mc1/level-042.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(f).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let t = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: t.height.clone(),
        tile_type: t.tile_type.clone(),
        shading: t.shading.clone().unwrap(),
        angle: t.angle.clone().unwrap(),
        ceiling: t.ceiling.clone().unwrap_or_default(),
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new(planes, &pkg.things.things, seed, assets);

    let cap = 1999usize;
    let idle = PlayerCommand::default();
    let mut peak = 0usize;
    let mut peak_tick = 0usize;
    let mut peak_comp: Vec<((u8, u8), usize)> = Vec::new();
    let mut total_dropped = 0u32;
    let mut frames_full = 0usize;

    for tick in 0..6000usize {
        // Orbit the mid-map so the player keeps sweeping the roaming
        // roster (radius 80 covers most of a 256 map's populated band).
        let a = tick as f32 * 0.008;
        let x = 128.0 + 80.0 * a.cos();
        let y = 128.0 + 80.0 * a.sin();
        let alt = w.ground_height_tiles(x, y) + 2.0;
        w.tick(PlayerPose::from_tiles(x, alt, y, a + 1.57, 0.0, 0.0), idle);
        w.take_teleport();

        let dropped = w.take_pool_exhausted();
        total_dropped += dropped;
        let (free, ev) = w.debug_pool();
        let live = cap.saturating_sub(free);
        if dropped > 0 {
            frames_full += 1;
        }
        if live > peak {
            peak = live;
            peak_tick = tick;
            let mut tally: BTreeMap<(u8, u8), usize> = BTreeMap::new();
            for e in &ev {
                *tally.entry((e.class, e.model)).or_default() += 1;
            }
            let mut v: Vec<_> = tally.into_iter().collect();
            v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            peak_comp = v;
        }
        if tick % 500 == 499 {
            println!(
                "t{:>4}: live {live:>4}/{cap}  free {free:>4}  dropped(this frame) {dropped:>3}",
                tick + 1
            );
        }
    }

    println!("\n=== summary ===");
    println!("peak live occupancy: {peak}/{cap} at t{peak_tick}");
    println!("frames with ANY drop: {frames_full}/6000");
    println!("total dropped over run: {total_dropped}");
    println!("\ncomposition at peak (top 16):");
    for ((c, m), n) in peak_comp.iter().take(16) {
        println!("  ({c:2},{m:3}) x{n}");
    }
}
