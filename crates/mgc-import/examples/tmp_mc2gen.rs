//! Dig instrument: regenerate one MC2 level's terrain and dump the
//! five planes as raw 256x256 byte images (`<out>/<plane>.gen`).
//!
//! This is the third side of the stock-bake triangle. `mgc-conform
//! terrain-diff --out` gives retail's measured record-0 planes and the
//! port's POST-LOAD planes; this gives the generator's output BEFORE
//! the MC2 load-time carve pass, which is what separates a generator
//! bug from a load-time-sculptor bug (docs/CONFORMANCE-FINDINGS.md,
//! MC2 cave stock-bake dig — the generator came out byte-perfect and
//! every divergence was in the carve).
//!
//! Usage: `cargo run -p mgc-import --example tmp_mc2gen -- <index> <out>`

use std::path::{Path, PathBuf};

use mgc_import::dattab::Archive;
use mgc_import::gamedata::Gamedata;
use mgc_import::level_mc2::Mc2Level;
use mgc_import::mc2_terrain;

const MC2_LEVEL_SIZE: usize = 26116;

fn main() {
    let mut a = std::env::args().skip(1);
    let index: u32 = a.next().expect("index").parse().expect("index");
    let out = PathBuf::from(a.next().expect("out dir"));
    let root = match std::env::var_os("MGC_GAMEDATA") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gamedata"),
    };
    let src = Gamedata::locate(&root).mc2.expect("mc2 gamedata");
    let archive = Archive::open(
        &src.read("LEVELS/LEVELS.DAT").unwrap(),
        &src.read("LEVELS/LEVELS.TAB").unwrap(),
    )
    .unwrap();
    let entry = archive
        .non_empty()
        .find(|e| e.index as u32 == index)
        .expect("entry");
    let payload = archive.extract(entry).unwrap();
    assert_eq!(payload.len(), MC2_LEVEL_SIZE);
    let level = Mc2Level::parse(&payload).unwrap();
    eprintln!(
        "{}: level {index} map_type {:?} basic_height {} gen {:?}",
        src.origin, level.header.map_type, level.header.basic_height, level.gen_map
    );
    let t = mc2_terrain::generate(&level);
    std::fs::create_dir_all(&out).unwrap();
    for (name, plane) in [
        ("type", &t.tile_type),
        ("height", &t.height),
        ("shading", &t.shading),
        ("angle", &t.angle),
        ("ceiling", &t.ceiling),
    ] {
        std::fs::write(out.join(format!("{name}.gen")), plane).unwrap();
    }
}
