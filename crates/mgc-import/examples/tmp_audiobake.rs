//! TEMP probe: run just the mc1-audio bundle bake into an out dir.
//! Usage: tmp_audiobake <gamedata-root> <out-dir>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, out] = &args[..] else {
        eprintln!("usage: tmp_audiobake <gamedata-root> <out-dir>");
        std::process::exit(2);
    };
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(root));
    let src = gd.mc1.expect("mc1 source");
    let t = std::time::Instant::now();
    let outputs =
        mgc_import::bundle::bake_mc1_audio(&src, std::path::Path::new(out)).expect("bake");
    println!("{} members in {:?}", outputs.len(), t.elapsed());
    for (name, _) in &outputs {
        println!("  {name}");
    }
}
