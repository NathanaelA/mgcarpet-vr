//! Dev aid: render one MC1 song's GM (-2) arrangement via oxisynth and
//! report stats. Usage: gm_probe <gamedata-root> <bank> <song-name>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, bank, song_name] = &args[..] else {
        eprintln!("usage: gm_probe <gamedata-root> <bank> <name>");
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
        "{name}: {} events, end_tick {}, tick_rate {}",
        song.events.len(),
        song.end_tick,
        song.tick_rate
    );
    let layered = mgc_import::adlib::has_danger_layer(&song);
    let mix = if layered {
        mgc_import::adlib::MixSpec::ambient()
    } else {
        mgc_import::adlib::MixSpec::full()
    };
    let midi = mgc_import::smf::encode(&song, &mix);
    let r = mgc_import::synth::GmRenderer::locate().expect("soundfont");
    println!("soundfont: {}", r.soundfont.display());
    let t0 = std::time::Instant::now();
    let pcm = r.render(&midi, 44100).unwrap();
    let frames = pcm.len() / 2;
    let peak = pcm.iter().fold(0f32, |m, s| m.max(s.abs()));
    let rms = (pcm.iter().map(|s| (s * s) as f64).sum::<f64>() / pcm.len() as f64).sqrt();
    println!(
        "rendered {frames} frames ({:.1}s) in {:?}; peak {peak:.3}, rms {rms:.4}, layered {layered}",
        frames as f32 / 44100.0,
        t0.elapsed()
    );
}
