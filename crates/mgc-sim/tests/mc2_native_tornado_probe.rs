//! NATIVE-RUN PROBE (dig instrument, not a law): level-001's
//! level-start arm tornado versus the live carpet, through the app's
//! own construction path (`new_full_env` + the MC2 setters) and the
//! app's own `Simulation::step`. Prints the carpet pose per tick.

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use mgc_sim::{FlightInput, Flyer, Simulation, ThrustModel};
use std::path::Path;

fn load(level: &str) -> Option<(World, mgc_formats::LevelPackage)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).ok()?;
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
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).ok()?;
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
    let mut w = World::new_full_env(
        planes,
        &pkg.things.things,
        seed,
        assets,
        GameId::Mc2.chassis(),
        GameId::Mc2,
        night,
    );
    w.set_placeholders(true);
    w.set_mc2_night_shade(night);
    if let Some(st) = pkg.stages.as_ref() {
        let rows: Vec<(i8, i16, i16, i16)> = st
            .checkpoints
            .iter()
            .map(|c| (c.index, c.stage, c.x, c.y))
            .collect();
        w.set_mc2_stages(&rows);
        let vars: Vec<(i8, i8, u8, u8, u32)> = st
            .variables
            .iter()
            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
            .collect();
        w.set_mc2_stagevars(&vars);
    }
    Some((w, pkg))
}

fn heads(w: &World) -> Vec<(usize, u16, u16, i16, u16)> {
    w.debug_pool()
        .1
        .into_iter()
        .filter(|e| e.class == 10 && e.model == 22 && e.life >= 0)
        .map(|e| {
            let p = w.debug_ent_pose(e.slot).unwrap();
            (e.slot, p.0, p.1, p.2, e.id24)
        })
        .collect()
}

#[test]
#[ignore]
fn native_tornado_probe() {
    let Some((w, pkg)) = load("level-001") else {
        eprintln!("no baked mc2 data — skipped");
        return;
    };
    let start = pkg
        .things
        .things
        .iter()
        .find(|t| t.kind == mgc_formats::ThingKind::Entity && t.class == 3 && t.model == 4)
        .map(|t| (t.x as f32 + 0.5, t.y as f32 + 0.5))
        .expect("start marker");
    let ex = ((start.0 * 256.0) as u32 & 0xFFFF) as u16;
    let ey = ((start.1 * 256.0) as u32 & 0xFFFF) as u16;
    let gz = w.ground_z_engine(ex, ey);
    eprintln!("start marker tile {:?} ground {gz}", start);
    for t in &pkg.things.things {
        if t.class == 11 || (t.class == 10 && t.model == 22) {
            eprintln!(
                "thing slot {} kind {:?} ({},{}) at ({},{}) dis_id {} swi_id {} swi_sz {} parent {} child {} par3 {:?}",
                t.slot,
                t.kind,
                t.class,
                t.model,
                t.x,
                t.y,
                t.dis_id,
                t.swi_id,
                t.swi_sz,
                t.parent,
                t.child,
                t.par3
            );
        }
    }
    let mut sim = Simulation::with_world(w);
    sim.thrust_model = if std::env::var_os("PROBE_ENHANCED").is_some() {
        sim.altitude_model = mgc_sim::AltitudeModel::ExtendedLift;
        ThrustModel::Enhanced
    } else {
        ThrustModel::Mc1
    };
    sim.flyer = Flyer {
        x: start.0,
        y: gz as f32 / 256.0 + 1.0,
        z: start.1,
        yaw: 0.0,
        pitch: 0.0,
        ..Flyer::default()
    };
    sim.sync_carpet_from_flyer();
    if let Ok(v) = std::env::var("PROBE_AT") {
        let p: Vec<i32> = v.split(',').map(|s| s.parse().unwrap()).collect();
        sim.carpet.x = p[0] as u16;
        sim.carpet.y = p[1] as u16;
        sim.carpet.z = p[2] as i16;
        sim.carpet.yaw = p[3] as u16;
        sim.carpet.act_speed = 80;
        sim.carpet.tgt_speed = 80;
        sim.sync_flyer_from_carpet();
        eprintln!("PARKED at {:?}", p);
    }
    let forced: Option<u16> = std::env::var("PROBE_FORCE_TILES")
        .ok()
        .and_then(|v| v.parse().ok());
    let ticks: u64 = std::env::var("PROBE_TICKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let mut placed = false;
    let input = FlightInput {
        thrust: if std::env::var_os("PROBE_ENHANCED").is_some() {
            1.0
        } else {
            0.0
        },
        ..FlightInput::default()
    };
    for t in 0..ticks {
        sim.step(&input);
        let w = sim.world.as_ref().unwrap();
        let hs = heads(w);
        if let (Some(tiles), false, Some(h)) = (forced, placed, hs.first()) {
            // Park the carpet `tiles` east of the funnel, facing north.
            sim.carpet.x = h.1.wrapping_add(tiles * 256);
            sim.carpet.y = h.2;
            sim.carpet.z = h.3 + 300;
            sim.carpet.yaw = 0;
            sim.sync_flyer_from_carpet();
            placed = true;
            eprintln!(
                "t={t} PLACED carpet at ({}, {}) next to head {:?}",
                sim.carpet.x, sim.carpet.y, h
            );
        }
        let c = sim.carpet;
        if hs.is_empty() && t % 20 != 0 {
            continue;
        }
        let f = sim.flyer;
        eprintln!(
            "t={t} carpet ({}, {}, {}) yaw {} act {} tgt {} strafe {} roll_f {} | flyer ({:.1}, {:.1}, {:.1}) yaw {:.2} v ({:.2}, {:.2}) | heads {:?}",
            c.x,
            c.y,
            c.z,
            c.yaw,
            c.act_speed,
            c.tgt_speed,
            c.strafe,
            c.roll_f,
            f.x,
            f.z,
            f.y,
            f.yaw,
            f.vx,
            f.vz,
            hs
        );
    }
}
