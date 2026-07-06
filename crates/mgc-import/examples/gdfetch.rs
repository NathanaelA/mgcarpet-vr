//! Dev aid: copy a file out of a gamedata source (CD image or overlay)
//! to disk. Usage: gdfetch <gamedata-root> <mc1|mc2> <REL/PATH> <out>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, game, rel, out] = &args[..] else {
        eprintln!("usage: gdfetch <gamedata-root> <mc1|mc2> <REL/PATH> <out>");
        std::process::exit(2);
    };
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(root));
    let src = match game.as_str() {
        "mc2" => gd.mc2,
        _ => gd.mc1,
    }
    .expect("game source");
    let data = src.read(rel).expect("read");
    std::fs::write(out, &data).unwrap();
    println!("{rel} -> {out} ({} bytes)", data.len());
}
