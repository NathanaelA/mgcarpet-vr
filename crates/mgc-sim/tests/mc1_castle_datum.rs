//! MC1 castle terrain DATUM law, adjudicated 2026-07-22 against the
//! decompile (remc1, corroborated byte-identical in remc1hw):
//!
//! The datum is NOT frozen at first build. On EVERY transform (first
//! build and each upgrade) the m41 leveler re-derives it as the
//! floor-mean of the four corners of the buildTab[level] footprint
//! grown by one tile per side (sub_28200 :30434 → sub_361C0), clamped
//! to 220, translates the whole footprint to it, and overwrites the
//! castle's site datum (+154, whose ONLY writer in the binary is the
//! leveler finish :30424). The painter stamps no apron — the sampled
//! corners are always virgin outside terrain. Consequence, faithful
//! to retail: a castle built on a peak SINKS toward ambient as the
//! footprint jumps (8 → 21 → 35 → 48 per side) reach downhill ground.
//! MC2 redesigned this (datum computed once at the ctor, perimeter-min,
//! frozen for life) — MC2 does not sink.
//!
//! A player report claims retail MC1 held a peak castle's height
//! through upgrades; the decompile refutes the mechanism, so any
//! frozen-datum change is a deliberate deviation awaiting a player
//! ruling (retail replay). Until then this test pins the FAITHFUL
//! law: datum after each transform == the corner mean sampled just
//! before it, and the peak castle demonstrably sinks.
//!
//! Self-skips when the baked tree is absent (game data is optional).

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::mc1::spells::SpellId;
use std::path::PathBuf;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    p.join("mc1/level-005.mgcl").exists().then_some(p)
}

fn load_parts(root: &std::path::Path) -> (Planes, Vec<mgc_formats::Thing>, u32, FeatureAssets) {
    let file = std::fs::File::open(root.join("mc1/level-005.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: Vec::new(),
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    (planes, pkg.things.things.clone(), seed, assets)
}

/// First tile whose neighborhood — including the cast site 16 tiles
/// south — is dry and free of the building-protection bit (the same
/// admission the spell_castle test uses).
fn clear_spot(planes: &Planes) -> (u16, u16) {
    for cy in (24..222u16).step_by(3) {
        'cand: for cx in (24..232u16).step_by(3) {
            for dy in -9i32..=25 {
                for dx in -9i32..=9 {
                    let t =
                        ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256);
                    if planes.angle[t] & 0x80 != 0 || planes.angle[t] & 0xF == 0 {
                        continue 'cand;
                    }
                }
            }
            return (cx, cy);
        }
    }
    panic!("no clear 19x19 spot on the level");
}

/// Raise a cone peak centered on (cx, cy): +3 height per tile of
/// closeness inside radius 13 (≈ +39 at the summit), so every ring a
/// growing castle footprint can reach sits on a real slope.
fn sculpt_peak(planes: &mut Planes, cx: u16, cy: u16) {
    for dy in -13i32..=13 {
        for dx in -13i32..=13 {
            let r = dx.abs().max(dy.abs());
            let lift = 3 * (13 - r);
            let t = ((cy as i32 + dy) as usize % 256) * 256 + ((cx as i32 + dx) as usize % 256);
            planes.height[t] = (planes.height[t] as i32 + lift).min(210) as u8;
        }
    }
}

fn castle_tile(w: &World) -> Option<(u8, u8)> {
    w.debug_pool()
        .1
        .into_iter()
        .find(|e| e.class == 3 && e.model == 2)
        .map(|e| (e.tx, e.ty))
}

/// The leveler's target law (sub_28200 :30434): floor-mean of the four
/// corners of the w×h footprint centered on the castle, grown by one
/// tile per side, clamped to 220.
fn grown_corner_mean(w: &World, tx: u8, ty: u8, fw: u8, fh: u8) -> i32 {
    let x0 = tx.wrapping_sub(fw / 2).wrapping_sub(1);
    let y0 = ty.wrapping_sub(fh / 2).wrapping_sub(1);
    let h = |x: u8, y: u8| w.planes().height[(y as usize) * 256 + x as usize] as u32;
    let sum = h(x0, y0)
        + h(x0.wrapping_add(fw + 2), y0)
        + h(x0.wrapping_add(fw + 2), y0.wrapping_add(fh + 2))
        + h(x0, y0.wrapping_add(fh + 2));
    ((sum >> 2) as i32).min(220)
}

#[test]
fn castle_datum_reaverages_to_the_grown_corner_mean_each_transform() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let (mut planes, things, seed, assets) = load_parts(&root);
    let dims: Vec<(u8, u8)> = assets.build_tab.iter().map(|b| (b.w, b.h)).collect();
    let (cx, cy) = clear_spot(&planes);
    sculpt_peak(&mut planes, cx, cy);
    let mut w = World::new(planes, &things, seed, assets);
    w.set_dev_spells(true);

    let px = cx as f32 + 0.5;
    let pz = cy as f32 + 16.5;
    // Hover ABOVE summit height: the ball's pitch eases toward the
    // ground at the target, so an approach from above grounds ON the
    // summit instead of skimming over it.
    let alt = w.ground_height_tiles(cx as f32 + 0.5, cy as f32 + 0.5) + 6.0;
    let pose = PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0);

    // Build L1 on the summit.
    w.tick(
        pose,
        PlayerCommand {
            equip_left: Some(SpellId(16)),
            ..Default::default()
        },
    );
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    for _ in 0..160 {
        w.tick(pose, PlayerCommand::default());
    }
    let (btx, bty) = castle_tile(&w).expect("L1 castle raised on the peak");
    assert!(
        (btx as i32 - cx as i32).abs() <= 3 && (bty as i32 - cy as i32).abs() <= 3,
        "castle landed on the summit (built at ({btx}, {bty}), peak ({cx}, {cy}))"
    );
    let datum = |w: &World| (w.debug_castle_site_z().expect("castle site datum") / 32) as i32;
    let (_, _, lvl) = w.loadout().castle.expect("castle panel");
    assert_eq!(lvl, 1, "castle established at level 1");
    let d1 = datum(&w);
    let mut trajectory = vec![(1u32, d1)];

    // Upgrade to the max (level 7). Before each cast, sample the four
    // grown-rect corners of the NEXT level's footprint — the painter
    // never writes them and the previous smooth already ran, so they
    // hold exactly what the leveler will average.
    for want in 2..=7u32 {
        let (fw, fh) = dims[want as usize % dims.len()];
        let expect = grown_corner_mean(&w, btx, bty, fw, fh);
        w.tick(pose, PlayerCommand::default()); // button release edge
        w.tick(
            pose,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        for _ in 0..300 {
            w.tick(pose, PlayerCommand::default());
        }
        let (_, _, lvl) = w.loadout().castle.expect("castle survives the upgrade");
        assert_eq!(lvl as u32, want, "upgrade {want} completed");
        let d = datum(&w);
        assert_eq!(
            d, expect,
            "level-{want} datum == the grown-corner floor-mean sampled pre-cast \
             (trajectory so far {trajectory:?})"
        );
        trajectory.push((want, d));
    }
    eprintln!("datum trajectory (level, height): {trajectory:?}");

    // The faithful consequence on a peak: the castle SINKS. If a
    // deliberate frozen-datum deviation ever lands (pending the
    // player's retail-replay ruling), this assertion must be
    // consciously inverted alongside it.
    let d7 = trajectory.last().unwrap().1;
    assert!(
        d7 < d1,
        "peak castle sank with footprint growth (L1 datum {d1}, L7 datum {d7})"
    );
}
