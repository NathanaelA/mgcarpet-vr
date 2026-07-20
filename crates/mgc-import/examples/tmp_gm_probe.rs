//! TEMP probe: compare MUSIC<bank>-0 (AdLib) vs -2 (General MIDI)
//! arrangements — channels, programs, CC7, velocities, danger layers.
//! Usage: tmp_gm_probe <gamedata-root>
use mgc_import::hmp::EventKind;

fn main() {
    let root = std::env::args().nth(1).expect("gamedata root");
    let gd = mgc_import::gamedata::Gamedata::locate(std::path::Path::new(&root));
    let src = gd.mc1.expect("mc1 source");
    let read = |rel: &str| -> Vec<u8> {
        let raw = src.read(rel).expect(rel);
        if mgc_import::rnc::is_rnc(&raw) {
            mgc_import::rnc::decompress(&raw).expect(rel)
        } else {
            raw
        }
    };
    for bank in 0..=1u32 {
        for driver in [0u32, 2] {
            let dat = read(&format!("DATA/MUSIC{bank}-{driver}.DAT"));
            let tab = read(&format!("DATA/MUSIC{bank}-{driver}.TAB"));
            let parsed = mgc_import::sound::parse_bank(bank, &tab, &dat, false).unwrap();
            for (_, name, hmp) in &parsed.entries {
                let song = match mgc_import::hmp::parse(hmp) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("bank{bank} drv{driver} {name}: PARSE ERROR {e}");
                        continue;
                    }
                };
                let mut notes = [0u32; 16];
                let mut vel_min = [255u8; 16];
                let mut vel_max = [0u8; 16];
                let mut progs: Vec<(u8, u8)> = Vec::new();
                let mut cc7: Vec<(u8, u8)> = Vec::new();
                let mut other_cc: Vec<(u8, u8)> = Vec::new();
                for ev in &song.events {
                    match ev.kind {
                        EventKind::NoteOn { ch, vel, .. } if vel > 0 => {
                            notes[ch as usize] += 1;
                            vel_min[ch as usize] = vel_min[ch as usize].min(vel);
                            vel_max[ch as usize] = vel_max[ch as usize].max(vel);
                        }
                        EventKind::Program { ch, prog } => {
                            if !progs.contains(&(ch, prog)) {
                                progs.push((ch, prog));
                            }
                        }
                        EventKind::Control { ch, ctrl: 7, val } => {
                            if !cc7.contains(&(ch, val)) {
                                cc7.push((ch, val));
                            }
                        }
                        EventKind::Control { ch, ctrl, .. } if !other_cc.contains(&(ch, ctrl)) => {
                            other_cc.push((ch, ctrl));
                        }
                        _ => {}
                    }
                }
                println!(
                    "bank{bank} drv{driver} {name}: {} ev, {} ticks @{}",
                    song.events.len(),
                    song.end_tick,
                    song.tick_rate
                );
                for ch in 0..16 {
                    if notes[ch] > 0 {
                        println!(
                            "  ch{ch:2}: {:5} notes vel {}..{}",
                            notes[ch], vel_min[ch], vel_max[ch]
                        );
                    }
                }
                println!("  progs {progs:?}");
                println!("  cc7 {cc7:?}");
                println!("  other_cc {other_cc:?}");
            }
        }
    }
}
