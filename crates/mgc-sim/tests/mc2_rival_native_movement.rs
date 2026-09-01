//! NATIVE-PLAY RIVAL MOVEMENT (dig D3 pin).
//!
//! `mc2_rival_approach`'s boost arm (sub_14C90 EF:6729-46) never
//! writes `cmd_speed` itself: outside the boost ring it casts SPEED
//! and leaves the speed to the token body `GetScroll_69DB0`
//! (EF:56216-63), which writes `speed_0xc_12 = sign * minSpeed *
//! (subSpellIndex + first)` every armed tick and `sign * minSpeed`
//! when the window closes. That body is dispatched at the TOKEN's own
//! class-15 slot, i.e. through [`World::mc2_manifestation_pass`] —
//! and that pass is gated on `mc2_token_slot_dispatch()`, which is
//! `mc2_carpet_slot != 0`, i.e. CONFORMANCE REPLAY ONLY. The rival
//! caster-slot stand-in `mc2_rival_buffs` skips spell 3 outright.
//!
//! So in native play nothing ever wrote a rival's commanded speed and
//! every rival sat on its spawn marker forever, waking only when the
//! human came inside the approach ring. This asserts the opposite.
//!
//! Runs against the real bakes (`baked/mc2`); skips silently when the
//! player's gamedata bake is absent (CI without game assets).

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

/// The app's MC2 level-start recipe (`WorldInit::build`), for one
/// level: bundle variant by header, stages + stagevars, wizards.
fn load(level: &str) -> Option<World> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let file = std::fs::File::open(root.join("mc2").join(format!("{level}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    let header = pkg.header.as_ref();
    let variant = match header.map(|h| (h.map_type, h.gfx_type)) {
        Some((mgc_formats::MapType::Night, g)) if g & 2 != 0 => "mc2-night-fog",
        Some((mgc_formats::MapType::Night, _)) => "mc2-night",
        Some((mgc_formats::MapType::Cave, _)) => "mc2-cave",
        _ => "mc2-day",
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets").join(variant)).ok()?;
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
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    w.set_mc2_night_shade(matches!(
        header.map(|h| h.map_type),
        Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
    ));
    w.set_mc2_doom_level(header.is_some_and(|h| h.gfx_type & 2 != 0));
    w.set_mc2_castle_purge_level(header.is_some_and(|h| h.gfx_type & 4 != 0));
    if let Some(st) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = st
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        if !rows.is_empty() {
            w.set_mc2_stages(&rows);
        }
        let vars: Vec<(i8, i8, u8, u8, u32)> = st
            .variables
            .iter()
            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
            .collect();
        if !vars.is_empty() {
            w.set_mc2_stagevars(&vars);
        }
    }
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some(w)
}

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

/// Live rival carpets as `(slot, tile_x, tile_y)` — class 3 model 1.
fn carpets(w: &World) -> Vec<(usize, u8, u8)> {
    let mut v: Vec<(usize, u8, u8)> = w
        .debug_pool()
        .1
        .into_iter()
        .filter(|e| e.class == 3 && e.model == 1 && e.life >= 0)
        .map(|e| (e.slot, e.tx, e.ty))
        .collect();
    v.sort();
    v
}

/// Torus tile distance on the 256x256 map.
fn tdist(a: (u8, u8), b: (u8, u8)) -> f64 {
    let d = |p: u8, q: u8| {
        let raw = (p as i32 - q as i32).abs();
        raw.min(256 - raw) as f64
    };
    (d(a.0, b.0).powi(2) + d(a.1, b.1).powi(2)).sqrt()
}

/// mc2:22 — seven authored rivals, every one of them max-Reflexes
/// (think EVERY tick) and holding the SPEED spell. Started fresh
/// with the human parked in the far corner, every rival must LEAVE
/// its start marker: the SPEED token body is what publishes their
/// commanded speed, and with it dead they sat on the flag forever
/// (measured 0.0-2.1 tiles over 1200 ticks; with the stand-in,
/// 40.3-193.8 tiles over the same window).
/// NON-VACUITY: `MGC_NO_RIVAL_SPEED_STANDIN=1` restores the
/// pre-law dispatch and this test goes RED (max 0.0 tiles).
#[test]
fn mc2_native_rivals_leave_their_start_markers() {
    let Some(mut w) = load("level-022") else {
        eprintln!("skipping: no baked/mc2 tree");
        return;
    };
    let start = carpets(&w);
    assert_eq!(start.len(), 7, "mc2:22 spawns seven AI wizards");
    // The human idles in the far corner — well outside every rival's
    // engagement ring, so nothing is woken by proximity.
    let pose = PlayerPose::from_tiles(2.0, 20.0, 2.0, 0.0, 0.0, 0.0);
    let idle = PlayerCommand::default();
    let mut reach: Vec<f64> = vec![0.0; start.len()];
    for _ in 0..1200 {
        w.tick(pose, idle);
        for (k, &(slot, sx, sy)) in start.iter().enumerate() {
            if let Some(&(_, x, y)) = carpets(&w).iter().find(|c| c.0 == slot) {
                let d = tdist((sx, sy), (x, y));
                if d > reach[k] {
                    reach[k] = d;
                }
            }
        }
    }
    for (k, &(slot, ..)) in start.iter().enumerate() {
        assert!(
            reach[k] > 10.0,
            "rival at slot {slot} never left its start marker \
             (max {:.1} tiles in 1200 ticks); reach = {reach:?}",
            reach[k]
        );
    }
}
