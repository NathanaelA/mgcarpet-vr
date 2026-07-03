//! Integration test: bake real MC1 levels, re-read the packages, and
//! verify they faithfully mirror a fresh parse of the source data.
//! Self-skips without game data.

use std::path::{Path, PathBuf};

use mgc_formats::{Game, ThingKind, mgcl};
use mgc_import::bake::bake_mc1_archive;
use mgc_import::dattab::Archive;
use mgc_import::level_mc1::Mc1Level;

fn gamedata_dir() -> PathBuf {
    match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    }
}

#[test]
fn baked_packages_round_trip_real_levels() {
    let base = gamedata_dir().join("mc1/LEVELS");
    let (dat, tab) = (base.join("LEVELS.DAT"), base.join("LEVELS.TAB"));
    if !dat.exists() {
        eprintln!("note: MC1 level data not present — skipping");
        return;
    }

    let out = std::env::temp_dir().join(format!("mgc-bake-test-{}", std::process::id()));
    let outputs = bake_mc1_archive(Game::MagicCarpet1, "mc1", &dat, &tab, &out).unwrap();
    assert_eq!(outputs.len(), 70);

    // Determinism: baking again yields identical hashes.
    let again = bake_mc1_archive(Game::MagicCarpet1, "mc1", &dat, &tab, &out).unwrap();
    assert_eq!(outputs, again);

    // Faithfulness: reload each package and compare against a fresh parse.
    let archive =
        Archive::open(&std::fs::read(&dat).unwrap(), &std::fs::read(&tab).unwrap()).unwrap();
    for entry in archive.non_empty() {
        let level = Mc1Level::parse(&archive.extract(entry).unwrap()).unwrap();
        let path = out.join(format!("mc1/level-{:03}.mgcl", entry.index));
        let package = mgcl::read(std::fs::File::open(&path).unwrap()).unwrap();

        assert_eq!(package.meta.game, Game::MagicCarpet1);
        assert_eq!(package.meta.level, entry.index as u32);

        let gen_params = package
            .gen_params
            .expect("game-derived packages carry genparams");
        assert_eq!(gen_params.seed, level.gen_map.seed);
        assert_eq!(gen_params.raise, level.gen_map.raise);
        assert_eq!(gen_params.footer, Some(level.footer));

        let entities = package
            .things
            .things
            .iter()
            .filter(|t| t.kind == ThingKind::Entity)
            .count();
        assert_eq!(entities, level.active_things().count());
        let markers = package
            .things
            .things
            .iter()
            .filter(|t| t.kind == ThingKind::Marker)
            .count();
        assert_eq!(markers, level.markers().count());

        // Spot-check field fidelity on every record via slot lookup.
        for thing in &package.things.things {
            let source = &level.things[thing.slot as usize];
            assert_eq!(
                (
                    thing.class,
                    thing.model,
                    thing.x,
                    thing.y,
                    thing.parent,
                    thing.child
                ),
                (
                    source.class,
                    source.model,
                    source.x,
                    source.y,
                    source.parent,
                    source.child
                ),
                "level {} slot {} diverges",
                entry.index,
                thing.slot
            );
        }
    }

    std::fs::remove_dir_all(&out).ok();
}
