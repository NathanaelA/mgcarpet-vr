//! Dev aid: render one MC1 song to WAV + print level stats.
//! Usage: music_probe <gamedata-root> <bank> <song-name> <out.wav>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, bank, song_name, out] = &args[..] else {
        eprintln!("usage: music_probe <gamedata-root> <bank> <name> <out.wav>");
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
    let inst = mgc_import::adlib::parse_bnk(&read("DATA/INST.BNK")).unwrap();
    let drum = mgc_import::adlib::parse_bnk(&read("DATA/DRUM.BNK")).unwrap();
    let dat = read(&format!("DATA/MUSIC{bank}-0.DAT"));
    let tab = read(&format!("DATA/MUSIC{bank}-0.TAB"));
    let parsed = mgc_import::sound::parse_bank(bank.parse().unwrap(), &tab, &dat, false).unwrap();
    let (_, name, hmp) = parsed
        .entries
        .iter()
        .find(|(_, n, _)| n.starts_with(song_name.as_str()))
        .expect("song not found");
    let song = mgc_import::hmp::parse(hmp).unwrap();
    println!(
        "{name}: {} events, {} ticks ({}s at {} Hz tick rate)",
        song.events.len(),
        song.end_tick,
        song.end_tick / song.tick_rate,
        song.tick_rate
    );
    let rate = 44100u32;
    let pcm = mgc_import::adlib::render(
        &song,
        &inst,
        &drum,
        rate,
        &mgc_import::adlib::MixSpec::full(),
    )
    .unwrap();
    let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap();
    let rms = (pcm
        .iter()
        .map(|&s| f64::from(s) * f64::from(s))
        .sum::<f64>()
        / pcm.len() as f64)
        .sqrt();
    println!("{} samples, peak {peak}, rms {rms:.0}", pcm.len());

    // Minimal WAV writer (16-bit mono).
    let mut w = Vec::new();
    let data_len = (pcm.len() * 2) as u32;
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in &pcm {
        w.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(out, &w).unwrap();
    println!("wrote {out}");
}
