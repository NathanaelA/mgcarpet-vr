//! Level-032 flying-archer probe: walk the trigger chain like
//! dbg032chain, then park and report per-model altitude above ground
//! and dispersal — identifying which entities fly/spread.

use mgc_sim::mc1::features::{FeatureAssets, Planes};
use mgc_sim::mc1::world::{PlayerCommand, PlayerPose, World};
use std::collections::HashSet;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from("baked");
    let file = std::fs::File::open(root.join("mc1/level-032.mgcl")).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc1-temperate")).unwrap();
    let terrain = pkg.terrain.as_ref().unwrap();
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap();
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new(planes, &pkg.things.things, seed, assets);

    let mut visited: HashSet<(usize, u16, u8, u8)> = HashSet::new();
    let mut last_pos = (4.5f32, 213.5f32);
    for _round in 0..6 {
        let (_, ev) = w.debug_pool();
        let next = ev
            .iter()
            .filter(|e| e.class == 11 && e.state <= 12)
            .map(|e| (e.slot, e.tx, e.ty, e.id24))
            .find(|&(slot, tx, ty, dis)| !visited.contains(&(slot, dis, tx, ty)));
        let Some((slot, tx, ty, dis)) = next else {
            break;
        };
        visited.insert((slot, dis, tx, ty));
        let (x, z) = (tx as f32 + 0.5, ty as f32 + 0.5);
        last_pos = (x, z);
        println!("== visiting trigger slot {slot} (fires dis {dis}) at ({tx},{ty})");
        for _ in 0..24 {
            let alt = w.ground_height_tiles(x, z) + 0.5;
            w.tick(
                PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0),
                PlayerCommand::default(),
            );
            w.take_teleport();
        }
    }

    // Park near the last trigger and let the population act.
    let (x, z) = last_pos;
    for _ in 0..500 {
        let alt = w.ground_height_tiles(x, z) + 0.5;
        w.tick(
            PlayerPose::from_tiles(x, alt, z, 0.0, 0.0, 0.0),
            PlayerCommand::default(),
        );
        w.take_teleport();
    }

    // Per (class, model): count, altitude-above-ground stats, spread.
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(u8, u8), Vec<(f32, f32, f32, u16)>> = BTreeMap::new();
    let poses = w.live_poses();
    let (_, ev) = w.debug_pool();
    let states: BTreeMap<(u8, u8), Vec<u8>> = {
        let mut m: BTreeMap<(u8, u8), Vec<u8>> = BTreeMap::new();
        for e in &ev {
            m.entry((e.class, e.model)).or_default().push(e.state);
        }
        m
    };
    for p in &poses {
        let ground = w.ground_height_tiles(p.x, p.z);
        groups.entry((p.class, p.model)).or_default().push((
            p.alt - ground,
            p.x,
            p.z,
            p.type_index,
        ));
    }
    for ((class, model), list) in &groups {
        let n = list.len();
        let mut aboves: Vec<f32> = list.iter().map(|v| v.0).collect();
        aboves.sort_by(f32::total_cmp);
        let (min_a, med_a, max_a) = (aboves[0], aboves[n / 2], aboves[n - 1]);
        let (mut minx, mut maxx, mut minz, mut maxz) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for v in list {
            minx = minx.min(v.1);
            maxx = maxx.max(v.1);
            minz = minz.min(v.2);
            maxz = maxz.max(v.2);
        }
        let types: HashSet<u16> = list.iter().map(|v| v.3).collect();
        let mut st = states.get(&(*class, *model)).cloned().unwrap_or_default();
        st.sort_unstable();
        st.dedup();
        println!(
            "c{class}m{model}: n={n} alt-above-ground min/med/max = {min_a:.2}/{med_a:.2}/{max_a:.2} \
             spread x {minx:.0}..{maxx:.0} z {minz:.0}..{maxz:.0} types {types:?} states {st:?}",
        );
    }
}
