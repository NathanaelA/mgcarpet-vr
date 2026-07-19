//! Level 032 (the segmented portal maze): the entry portal (class 10
//! model 34, slot 569 at (11,253), two tiles from the player start)
//! must spawn at level init and teleport the player to its authored
//! destination (child/parent = (5.5, 230.5), the maze entrance).
//!
//! Self-skips when the baked tree is absent.

use mgc_sim::mc1::features::{FeatureAssets, Planes};
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    p.join("mc1/level-032.mgcl").exists().then_some(p)
}

fn build_world(root: &std::path::Path) -> World {
    let file = std::fs::File::open(root.join("mc1/level-032.mgcl")).unwrap();
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
    World::new(planes, &pkg.things.things, seed, assets)
}

#[test]
fn level_032_entry_portal_spawns_and_teleports() {
    let Some(root) = baked_root() else {
        common::golden_skip("baked data not present");
        return;
    };
    let mut w = build_world(&root);

    // The portal exists at level start.
    let portals: Vec<_> = w
        .live_things()
        .into_iter()
        .filter(|t| t.class == 10 && t.model == 34)
        .collect();
    assert_eq!(portals.len(), 1, "the entry portal spawns at init");
    assert_eq!((portals[0].x, portals[0].y), (11, 253));

    // The portal spawns 640 units up and its first tick must both
    // drop it to the ground AND flag the entity set dirty so the pose
    // consumer redraws it there — even with zero creatures ticking
    // (032's population is disposition-gated).
    let far = PlayerPose::from_tiles(100.0, 20.0, 100.0, 0.0, 0.0, 0.0);
    w.entities_dirty = false;
    w.tick(far, PlayerCommand::default());
    assert!(
        w.entities_dirty,
        "portal re-ground must flag the pose refresh"
    );
    let pose = w
        .live_poses()
        .into_iter()
        .find(|p| p.class == 10 && p.model == 34)
        .expect("portal pose");
    let pground = w.ground_height_tiles(pose.x, pose.z);
    assert!(
        (pose.alt - pground).abs() < 0.05,
        "portal sits on the ground: alt {} vs ground {pground}",
        pose.alt
    );

    // Hovering NEXT to the portal but facing away: no teleport.
    let ground = w.ground_height_tiles(11.5, 254.3);
    let facing_away =
        PlayerPose::from_tiles(11.5, ground + 0.5, 254.3, std::f32::consts::PI, 0.0, 0.0);
    for _ in 0..8 {
        w.tick(facing_away, PlayerCommand::default());
        assert!(w.take_teleport().is_none(), "facing away must not teleport");
    }

    // Flying INTO the vortex (facing north, toward it): teleported to
    // the authored destination (child=5, parent=230 → (5.5, 230.5)).
    let facing_portal = PlayerPose::from_tiles(11.5, ground + 0.5, 254.3, 0.0, 0.0, 0.0);
    let mut dest = None;
    for _ in 0..8 {
        w.tick(facing_portal, PlayerCommand::default());
        if let Some(d) = w.take_teleport() {
            dest = Some(d);
            break;
        }
    }
    let (dx, dz) = dest.expect("flying into the portal teleports");
    assert!(
        (dx - 5.5).abs() < 0.01 && (dz - 230.5).abs() < 0.01,
        "dest = ({dx}, {dz})"
    );

    // The portal is persistent (maxLife 0): it keeps working.
    let mut again = None;
    for _ in 0..8 {
        w.tick(facing_portal, PlayerCommand::default());
        if let Some(d) = w.take_teleport() {
            again = Some(d);
            break;
        }
    }
    assert!(again.is_some(), "the portal never expires");
}
