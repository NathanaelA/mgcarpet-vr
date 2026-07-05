//! Integration test: bake real MC1 levels, re-read the packages, and
//! verify they faithfully mirror a fresh parse of the source data.
//! Self-skips without game data.

use std::path::{Path, PathBuf};

use mgc_import::bake::{bake_mc2_archive, find_genlevel};

use mgc_formats::{Game, ThingKind, mgcl};
use mgc_import::bake::bake_mc1_archive;
use mgc_import::dattab::Archive;
use mgc_import::gamedata::Gamedata;
use mgc_import::level_mc1::Mc1Level;

fn gamedata() -> Gamedata {
    let root = match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    };
    Gamedata::locate(&root)
}

#[test]
fn baked_packages_round_trip_real_levels() {
    let Some(src) = gamedata().mc1 else {
        eprintln!("note: MC1 level data not present — skipping");
        return;
    };

    let out = std::env::temp_dir().join(format!("mgc-bake-test-{}", std::process::id()));
    let outputs = bake_mc1_archive(Game::MagicCarpet1, "mc1", &src, "LEVELS/LEVELS", &out).unwrap();
    assert_eq!(outputs.len(), 70);

    // Determinism: baking again yields identical hashes.
    let again = bake_mc1_archive(Game::MagicCarpet1, "mc1", &src, "LEVELS/LEVELS", &out).unwrap();
    assert_eq!(outputs, again);

    // Faithfulness: reload each package and compare against a fresh parse.
    let archive = Archive::open(
        &src.read("LEVELS/LEVELS.DAT").unwrap(),
        &src.read("LEVELS/LEVELS.TAB").unwrap(),
    )
    .unwrap();
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

/// Entity-placement coherence canary (docs/ROADMAP.md "MC1 terrain
/// oracle"): level 001 generates ~82% water, yet placed trees/stones
/// (class 2) land on dry tiles — 158/159 with the MC2 oracle, and the
/// native MC1 generator must hold the same line (its shoreline detail
/// differs slightly: extra smoothing and shore-flattening passes).
/// Random placement would hit water ~4 times in 5, so a tolerance of 2
/// still fails loudly if the generator drifts. Skips without game data.
#[test]
fn mc1_terrain_generation_is_coherent() {
    let Some(src) = gamedata().mc1 else {
        eprintln!("note: MC1 level data not present — skipping");
        return;
    };

    let out = std::env::temp_dir().join(format!("mgc-mc1-terrain-test-{}", std::process::id()));
    bake_mc1_archive(Game::MagicCarpet1, "mc1", &src, "LEVELS/LEVELS", &out).unwrap();

    let package = mgcl::read(std::fs::File::open(out.join("mc1/level-001.mgcl")).unwrap()).unwrap();
    let terrain = package.terrain.expect("terrain baked");
    let water = terrain.height.iter().filter(|&&h| h == 0).count() as f64 / 65536.0;
    assert!(
        (0.70..0.90).contains(&water),
        "level 001 water fraction {water:.2} outside the known ~0.82"
    );
    let (mut total, mut wet) = (0u32, 0u32);
    for thing in &package.things.things {
        if thing.kind != ThingKind::Entity || thing.class != 2 {
            continue;
        }
        total += 1;
        if terrain.height[thing.y as usize * 256 + thing.x as usize] == 0 {
            wet += 1;
        }
    }
    assert!(total > 100, "expected >100 class-2 entities, got {total}");
    assert!(
        wet <= 2,
        "{wet}/{total} class-2 entities landed in water (known-good: 1/159)"
    );

    std::fs::remove_dir_all(&out).ok();
}

/// Oracle cross-check: bake MC2 with terrain and verify the heightmap of
/// entry 10 byte-matches remc2's DOSBox-verified regression fixture
/// (memimages level11 — one of the levels whose post-load state equals
/// pristine generation). Skips without game data, the oracle tool, or a
/// remc2 checkout (override location with MGC_REMC2).
#[test]
fn baked_terrain_matches_remc2_fixture() {
    let Some(src) = gamedata().mc2 else {
        eprintln!("note: MC2 level data not present — skipping");
        return;
    };
    let Some(tool) = find_genlevel().or_else(|| {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/mc2-genlevel/mc2-genlevel");
        p.exists().then_some(p)
    }) else {
        eprintln!("note: mc2-genlevel not built — skipping");
        return;
    };
    let remc2 = std::env::var_os("MGC_REMC2")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../remc2"));
    let fixture = remc2
        .join("remc2-regression-test/memimages/regressions/level11/sequence-002285FF-002DC4E0.bin");
    if !fixture.exists() {
        eprintln!("note: remc2 fixture not present — skipping");
        return;
    }

    let out = std::env::temp_dir().join(format!("mgc-oracle-test-{}", std::process::id()));
    let (outputs, _) = bake_mc2_archive(&src, &out, Some(&tool)).unwrap();
    assert_eq!(outputs.len(), 165);

    let package = mgcl::read(std::fs::File::open(out.join("mc2/level-010.mgcl")).unwrap()).unwrap();
    let terrain = package.terrain.expect("terrain baked");
    let reference = std::fs::read(&fixture).unwrap();
    assert_eq!(
        terrain.tile_type,
        &reference[0..0x10000],
        "tile type diverges from reference"
    );
    assert_eq!(
        terrain.height,
        &reference[0x10000..0x20000],
        "heightmap diverges from reference"
    );
    eprintln!("oracle terrain byte-matches remc2 fixture for entry 10");

    std::fs::remove_dir_all(&out).ok();
}
