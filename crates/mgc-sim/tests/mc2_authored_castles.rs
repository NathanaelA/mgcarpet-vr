//! Round 110 pin: the AUTHORED rival castles are stamped into the
//! GENERATED terrain at load (EF:43787-43800, one `sub_36FC0` pass per
//! row `0..castle_level`, cumulative). `mgc-conform terrain-check`
//! against the mc2l22 / mc2l4 / mc2l6-rival-spells-galore takes read
//! retail's record 0 with castle tiles (types 22/24/26/27) that the
//! port lacked: the port settled the (10,42) REPAINT painter at row
//! `level - 1`, which stamps nothing at castle level 1 and 332 of a
//! level-5 castle's 820 cells. The counts below are RETAIL's (a 33×33
//! box around each wizard start, the record-0 base of each take).
mod common;

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

fn load(level: &str, bank: &str) -> Option<(World, LevelPackage)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join(bank)).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.mc2_extent_dims(&root.join("assets")) {
        Some(dims) => assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims)),
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).ok()?,
        None => assets,
    };
    let file = std::fs::File::open(root.join("mc2").join(format!("{level}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let night = matches!(
        pkg.header.as_ref().map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    );
    let mut w = World::new_for_game_env(planes, &pkg.things.things, seed, assets, GameId::Mc2, night);
    w.set_placeholders(true);
    w.set_mc2_night_shade(night);
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some((w, pkg))
}

/// The app's / harness's `mc2_rival_configs` resolution (wizards.json +
/// the header's per-player authored castle levels).
fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(w), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
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
        out[slot] = Some(Mc2RivalConfig {
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
    (out, count)
}

/// The castle's FIRST TICK is part of the law (the level-up commit
/// paints BUILD00 row `castle_level`), and its painter lays the paint
/// codes on a cadence — a count before ~tick 8 still reads the mint's
/// stamps alone (16/0/332…). Settle 12 idle ticks, as `terrain-check`
/// settles by the recorder's phase (8-9 on the graded takes).
fn settle(w: &mut World, ticks: usize) {
    let (px, pz) = (128.5, 128.5);
    for _ in 0..ticks {
        let alt = w.ground_height_tiles(px, pz) + 2.0;
        let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);
        w.tick(pose, PlayerCommand::default());
    }
}

/// Castle tiles (types 22/24/26/27) inside the 33×33 box around a
/// wizard start — the same census `terrain-check` was clustered with.
fn castle_tiles(w: &World, x: i32, y: i32) -> usize {
    let p = w.planes_clone();
    let mut n = 0;
    for dy in -16..=16 {
        for dx in -16..=16 {
            let t = (((y + dy) & 255) as usize) * 256 + ((x + dx) & 255) as usize;
            if matches!(p.tile_type[t], 22 | 24 | 26 | 27) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn mc2_level_022_authored_rival_castles_are_stamped_at_load() {
    let Some((mut w, _)) = load("level-022", "mc2-day") else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    settle(&mut w, 12);
    // header players = [0, 2, 1, 5, 5, 5, 6, 7]: (3,5)@(175,156) level 2,
    // (3,6)@(207,157) level 1, (3,7)@(66,99) / (3,8)@(193,43) /
    // (3,9)@(68,158) level 5, (3,10)@(193,99) level 6, (3,11)@(68,43)
    // level 7. Retail record 0 (mc2l22.mgcr, phase 9).
    let got = [
        castle_tiles(&w, 175, 156),
        castle_tiles(&w, 207, 157),
        castle_tiles(&w, 66, 99),
        castle_tiles(&w, 193, 43),
        castle_tiles(&w, 68, 158),
        castle_tiles(&w, 193, 99),
        castle_tiles(&w, 68, 43),
    ];
    assert_eq!(
        got,
        [16, 16, 820, 820, 820, 820, 838],
        "mc2:22 authored castle tiles per rival (retail; the (10,42) repaint at row level-1 \
         gave [16, 0, 332, 332, 332, 820, 820]; before the first tick the stamps alone read the same)"
    );
}

#[test]
fn mc2_level_006_authored_level_1_castle_is_a_16_tile_stump() {
    let Some((mut w, _)) = load("level-006", "mc2-night") else {
        common::golden_skip("baked mc2 data not present");
        return;
    };
    settle(&mut w, 12);
    // header players = [0, 0, 0, 1, ...]: only (3,7)@(241,162) starts
    // with a castle, level 1 = BUILD00 row 0 alone (a real 4×4 row on
    // MC2 — the MC1 note "row 0 is empty" does not carry).
    assert_eq!(
        castle_tiles(&w, 241, 162),
        16,
        "mc2:6 (3,7) level-1 authored castle (retail 16; 0 before round 110)"
    );
    assert_eq!(castle_tiles(&w, 65, 94), 0, "mc2:6 (3,6) has no authored castle");
}
