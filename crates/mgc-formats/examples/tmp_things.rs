//! Dump one baked level's authored THING records as `SCULPT thing` lines.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = format!(
        "baked/{}/level-{:03}.mgcl",
        a[1],
        a[2].parse::<u32>().unwrap()
    );
    let pkg: mgc_formats::LevelPackage =
        mgc_formats::mgcl::read(std::fs::File::open(&p).unwrap()).unwrap();
    eprintln!(
        "header={:?}",
        pkg.header.as_ref().map(|h| (h.map_type, h.gfx_type))
    );
    for t in &pkg.things.things {
        println!(
            "SCULPT thing slot={} ({},{}) x={} y={} dis={} swi_sz={} swi_id={} parent={} child={} par3={} kind={:?}",
            t.slot,
            t.class,
            t.model,
            t.x,
            t.y,
            t.dis_id,
            t.swi_sz,
            t.swi_id,
            t.parent,
            t.child,
            t.par3.unwrap_or(0),
            t.kind
        );
    }
}
