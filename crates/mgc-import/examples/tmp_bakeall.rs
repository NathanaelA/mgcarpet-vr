//! TEMP probe: run the full startup bake (bake_all — what the game's
//! bakecheck runs) into an out dir, surfacing every note/error.
//! Usage: tmp_bakeall <gamedata-root> <out-dir>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, out] = &args[..] else {
        eprintln!("usage: tmp_bakeall <gamedata-root> <out-dir>");
        std::process::exit(2);
    };
    let t = std::time::Instant::now();
    match mgc_import::bake::bake_all(std::path::Path::new(root), std::path::Path::new(out)) {
        Ok(_) => println!("BAKE OK in {:?}", t.elapsed()),
        Err(e) => println!("BAKE ERROR: {e}"),
    }
}
