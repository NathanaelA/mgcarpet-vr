fn main() {
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new("gamedata"));
    for f in gd.mc2.expect("mc2").list() {
        println!("{f}");
    }
}
