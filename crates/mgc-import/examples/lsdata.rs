fn main() {
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new("gamedata"));
    for f in gd.mc1.expect("mc1").list() {
        println!("{f}");
    }
}
