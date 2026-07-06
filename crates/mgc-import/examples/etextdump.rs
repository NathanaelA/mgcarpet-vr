//! Dev aid: dump ETEXT.DAT (or a localized variant) as numbered
//! strings — null-terminated, stored uncompressed (roadmap "Text"
//! track). Usage: etextdump <gamedata-root> [mc2] [FILENAME]
fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("gamedata root");
    let rest: Vec<String> = args.collect();
    let mc2 = rest.iter().any(|a| a == "mc2");
    let file = rest
        .iter()
        .find(|a| a.as_str() != "mc2")
        .cloned()
        .unwrap_or_else(|| "DATA/ETEXT.DAT".to_string());

    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(&root));
    let src = if mc2 { gd.mc2 } else { gd.mc1 }.expect("game source");
    let mut data = src.read(&file).expect("read etext");
    if data.starts_with(b"RNC") {
        data = mgc_import::rnc::decompress(&data).expect("rnc");
    }
    for (i, s) in data.split(|&b| b == 0).enumerate() {
        if s.is_empty() {
            continue;
        }
        let txt: String = s
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
            .collect();
        println!("{i:4}: {txt}");
    }
}
