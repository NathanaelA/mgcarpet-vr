//! TEMP probe: dump one GM song as SMF files (full/ambient/stem).
//! Usage: tmp_midi_dump <gamedata-root> <bank> <name> <out-prefix>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, bank, song_name, out] = &args[..] else {
        eprintln!("usage: tmp_midi_dump <gamedata-root> <bank> <name> <out-prefix>");
        std::process::exit(2);
    };
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(root));
    let src = gd.mc1.expect("mc1 source");
    let read = |rel: &str| -> Vec<u8> {
        let raw = src.read(rel).expect(rel);
        if mgc_import::rnc::is_rnc(&raw) {
            mgc_import::rnc::decompress(&raw).expect(rel)
        } else {
            raw
        }
    };
    let dat = read(&format!("DATA/MUSIC{bank}-2.DAT"));
    let tab = read(&format!("DATA/MUSIC{bank}-2.TAB"));
    let parsed = mgc_import::sound::parse_bank(bank.parse().unwrap(), &tab, &dat, false).unwrap();
    let (_, name, hmp) = parsed
        .entries
        .iter()
        .find(|(_, n, _)| n.starts_with(song_name.as_str()))
        .expect("song not found");
    let song = mgc_import::hmp::parse(hmp).unwrap();
    println!(
        "{name}: {} events, {} ticks ({}s), danger={}",
        song.events.len(),
        song.end_tick,
        song.end_tick / song.tick_rate,
        mgc_import::adlib::has_danger_layer(&song)
    );
    let full = mgc_import::smf::encode(&song, &mgc_import::adlib::MixSpec::full());
    let ambient = mgc_import::smf::encode(&song, &mgc_import::adlib::MixSpec::ambient());
    let stem = mgc_import::smf::encode(&song, &mgc_import::adlib::MixSpec::danger_stem());
    std::fs::write(format!("{out}-full.mid"), full).unwrap();
    std::fs::write(format!("{out}-ambient.mid"), ambient).unwrap();
    std::fs::write(format!("{out}-stem.mid"), stem).unwrap();
    println!("wrote {out}-{{full,ambient,stem}}.mid");
}
