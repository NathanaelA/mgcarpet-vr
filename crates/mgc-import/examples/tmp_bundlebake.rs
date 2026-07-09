//! TEMP probe: run the mc1 graphics-bundle bake (temperate + arctic)
//! into an out dir, printing every warning.
//! Usage: tmp_bundlebake <gamedata-root> <out-dir>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, out] = &args[..] else {
        eprintln!("usage: tmp_bundlebake <gamedata-root> <out-dir>");
        std::process::exit(2);
    };
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(root));
    let src = gd.mc1.expect("mc1 source");
    let t = std::time::Instant::now();
    match mgc_import::bundle::bake_mc1_bundles(&src, std::path::Path::new(out)) {
        Ok(outputs) => println!("OK: {} members in {:?}", outputs.len(), t.elapsed()),
        Err(e) => println!("BAKE ERROR: {e}"),
    }
}
