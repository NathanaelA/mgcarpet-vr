//! TEMP probe: boot every baked cave level and verify the sculptor
//! band carved (planes differ from the baked foundation) and the
//! floor↔ceiling invariant holds post-settle (0 violations expected
//! since the Phase-4.5 session-2 load-time arms).
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;

fn main() {
    let root = std::path::Path::new("baked");
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-cave")).unwrap();
    let assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).unwrap(),
        None => assets,
    };

    let mut names: Vec<_> = std::fs::read_dir(root.join("mc2"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    names.sort();

    let mut caves = 0usize;
    let mut total_bad = 0usize;
    for path in names {
        let lvl = path.file_stem().unwrap().to_string_lossy().into_owned();
        let f = std::fs::File::open(&path).unwrap();
        let pkg = mgc_formats::mgcl::read(f).unwrap();
        let terrain = pkg.terrain.as_ref().unwrap();
        let Some(baked_ceiling) = terrain.ceiling.clone() else {
            continue;
        };
        caves += 1;
        let planes = Planes {
            height: terrain.height.clone(),
            tile_type: terrain.tile_type.clone(),
            shading: terrain.shading.clone().unwrap(),
            angle: terrain.angle.clone().unwrap(),
            ceiling: baked_ceiling.clone(),
        };
        let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
        let w = World::new_for_game(
            planes,
            &pkg.things.things,
            seed,
            assets.clone(),
            mgc_sim::ids::GameId::Mc2,
        );
        let t = w.planes();
        let ceil_diff = t
            .ceiling
            .iter()
            .zip(&baked_ceiling)
            .filter(|(a, b)| a != b)
            .count();
        let floor_diff = t
            .height
            .iter()
            .zip(&terrain.height)
            .filter(|(a, b)| a != b)
            .count();
        let sealed = t.angle.iter().filter(|a| **a & 8 != 0).count();
        let bad = (0..0x10000)
            .filter(|&i| {
                let open = t.ceiling[i] > t.height[i];
                open == (t.angle[i] & 8 != 0)
            })
            .count();
        total_bad += bad;
        println!(
            "{lvl}: carved ceiling cells {ceil_diff}, floor {floor_diff}, \
             sealed {sealed}/65536, invariant violations {bad}"
        );
    }
    println!("== {caves} cave levels, total invariant violations {total_bad}");
}
