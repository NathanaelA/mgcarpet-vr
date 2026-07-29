//! Diagnostic: dump mc1 rival AI internals to explain "follows target,
//! casts nothing". Usage: cargo run --example rival_cast_probe -- 024
use std::path::Path;

const SPELL_NAMES: [&str; 24] = [
    "Fireball",
    "Heal",
    "Accel",
    "Possess",
    "Shield",
    "BeyondSight",
    "Earthquake",
    "Meteor",
    "Volcano",
    "Crater",
    "Portal",
    "Duel",
    "Invis",
    "StealMana",
    "Rebound",
    "Lightning",
    "Castle",
    "Undead",
    "LStorm",
    "ManaMagnet",
    "FireStorm",
    "AccelBack",
    "MagicBomb",
    "RapidFire",
];

fn names(ids: &[usize]) -> String {
    ids.iter()
        .map(|&s| format!("{}({})", s, SPELL_NAMES[s]))
        .collect::<Vec<_>>()
        .join(",")
}

fn main() {
    let lvl = std::env::args().nth(1).unwrap_or("024".into());
    let f = std::fs::File::open(format!("baked/mc1/level-{lvl}.mgcl")).unwrap();
    let package = mgc_formats::mgcl::read(std::io::BufReader::new(f)).unwrap();
    let dir = Path::new("baked/assets/mc1-temperate");
    let bundle = mgc_formats::bundle::Bundle::load(dir).unwrap();
    let terrain = package.terrain.as_ref().unwrap();
    let assets = mgc_sim::engine::features::FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let planes = mgc_sim::engine::features::Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = package.gen_params.as_ref().unwrap().seed;
    let mut w = mgc_sim::engine::world::World::new(planes, &package.things.things, seed, assets);
    let wiz = package.wizards.as_ref().unwrap();
    let count = wiz.player_count.unwrap();
    let mut cfgs: [Option<mgc_sim::mc1::rivals::RivalConfig>; 8] = Default::default();
    for (slot, c) in wiz.wizards.iter().enumerate().take(8).skip(1) {
        let mut book = [false; 24];
        let mut allowed = [false; 24];
        let am = c.allowed_spells.as_ref().unwrap();
        for s in 0..24 {
            allowed[s] = am[s] != 0;
            book[s] = allowed[s] && c.starting_spells[s] != 0;
        }
        cfgs[slot] = Some(mgc_sim::mc1::rivals::RivalConfig {
            aggression: c.aggression as u8,
            accuracy: c.accuracy.unwrap() as u8,
            tempo: c.tempo.unwrap() as u8,
            castle_level: c.castle_level.unwrap(),
            book,
            allowed,
        });
    }
    w.set_wizards(&cfgs, count);
    println!(
        "=== mc1:{lvl}  players={count} rivals={} ===",
        w.rival_views().len()
    );

    // Park the player near the level start.
    let start = package
        .things
        .things
        .iter()
        .find(|t| t.class == 3 && t.model == 4)
        .unwrap();
    let pose = mgc_sim::engine::world::PlayerPose::level(
        ((start.x as u32) << 8) as u16 + 128,
        ((start.y as u32) << 8) as u16 + 128,
        3000,
        0,
    );

    // How many class-5 monsters carry mana (the HuntMana bait)?
    let count_mana_mobs = |w: &mgc_sim::engine::world::World| {
        let (_, pool) = w.debug_pool();
        pool.iter()
            .filter(|e| e.class == 5 && e.life >= 0 && e.flags & 0x400 == 0)
            .count()
    };
    println!("class-5 creatures alive at t0: {}", count_mana_mobs(&w));

    let dump = |w: &mgc_sim::engine::world::World, t: u32| {
        println!("--- t{t} ---");
        for d in w.debug_rival_ai() {
            println!(
                "  slot{} {:<12} tgt={:<6} off={} pov={} burst={:>4} mana={}/{} castle_stored={:?}",
                d.slot,
                d.state,
                d.target,
                d.has_offense,
                d.poverty,
                d.burst,
                d.mana,
                d.mana_max,
                d.castle_stored,
            );
            println!("        known:   [{}]", names(&d.known));
            println!("        owned:   [{}]", names(&d.owned));
        }
    };

    dump(&w, 0);
    for t in 0..12000u32 {
        w.tick(pose, mgc_sim::engine::world::PlayerCommand::default());
        if matches!(t + 1, 500 | 2000 | 6000 | 12000) {
            dump(&w, t + 1);
            println!("      class-5 alive: {}", count_mana_mobs(&w));
        }
    }
}
