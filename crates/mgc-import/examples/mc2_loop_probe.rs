//! Scan the G-section bank-1 gameplay tracks (C1GAME1/2/3) for XMI FOR loops
//! (cc116/117) using the same walk as engine/XmiInfo.cpp XMI_FindLoopEvents.
use std::path::Path;
fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn find_from(h: &[u8], n: &[u8], start: usize) -> Option<usize> {
    h[start..]
        .windows(n.len())
        .position(|w| w == n)
        .map(|i| i + start)
}
fn main() {
    let root = std::env::args().nth(1).unwrap();
    let gd = mgc_import::gamedata::Gamedata::locate(Path::new(&root));
    let src = gd.mc2.expect("mc2");
    let data = src.read("SOUND/MUSIC.DAT").unwrap();
    let datapos = le_u32(&data, data.len() - 4) as usize;
    let table = datapos + 8;
    // G driver=0. Scan BOTH banks (chan 0 = C2 set, chan 1 = C1 set).
    for chan in 0..2 {
        // G driver = 0 -> slot at table + chan*64 + drv*16 (drv 0).
        let rec = table + chan * 64;
        let xmi_off = le_u32(&data, rec + 4) as usize;
        let xmi_size = le_u32(&data, rec + 12) as usize;
        let xmi = &data[xmi_off..(xmi_off + xmi_size).min(data.len())];
        println!("\n### G bank {chan}: xmi {} bytes ###", xmi.len());
        // Walk each of the 6 FORM XMID subsongs by finding successive EVNT chunks.
        let mut search = 0;
        let mut sub = 0;
        while let Some(ev) = find_from(xmi, b"EVNT", search) {
            let sz = be32(xmi, ev + 4) as usize;
            let (start, end) = (ev + 8, (ev + 8 + sz).min(xmi.len()));
            let (loops, tempo) = scan_events(&xmi[start..end]);
            println!(
                "  subsong {sub}: EVNT@{ev} len={sz}  first_tempo={} us/qn ({} bpm)  loops={:?}",
                tempo,
                if tempo > 0 { 60_000_000 / tempo } else { 0 },
                loops
            );
            sub += 1;
            search = ev + 8 + sz + (sz & 1);
            if sub >= 6 {
                break;
            }
        }
    }
}
// returns (loop events as (cc,val), first tempo us/qn)
fn scan_events(ev: &[u8]) -> (Vec<(u8, u8)>, u32) {
    let mut p = 0;
    let mut loops = Vec::new();
    let mut tempo = 0u32;
    while p < ev.len() {
        while p < ev.len() && ev[p] & 0x80 == 0 {
            p += 1;
        } // delta
        if p >= ev.len() {
            break;
        }
        let st = ev[p];
        p += 1;
        match st {
            0xFF => {
                if p >= ev.len() {
                    break;
                }
                let meta = ev[p];
                p += 1;
                let mut l = 0u32;
                loop {
                    if p >= ev.len() {
                        break;
                    }
                    let b = ev[p];
                    p += 1;
                    l = (l << 7) | (b & 0x7f) as u32;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                if meta == 0x51 && l == 3 && p + 3 <= ev.len() {
                    tempo = ((ev[p] as u32) << 16) | ((ev[p + 1] as u32) << 8) | ev[p + 2] as u32;
                }
                p += l as usize;
                if meta == 0x2f {
                    break;
                }
            }
            0xF0 | 0xF7 => {
                let mut l = 0u32;
                loop {
                    if p >= ev.len() {
                        break;
                    }
                    let b = ev[p];
                    p += 1;
                    l = (l << 7) | (b & 0x7f) as u32;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                p += l as usize;
            }
            _ => match st & 0xF0 {
                0x80 | 0xA0 | 0xE0 => {
                    p += 2;
                }
                0xC0 | 0xD0 => {
                    p += 1;
                }
                0x90 => {
                    p += 2;
                    let mut _d = 0u32;
                    loop {
                        if p >= ev.len() {
                            break;
                        }
                        let b = ev[p];
                        p += 1;
                        _d = (_d << 7) | (b & 0x7f) as u32;
                        if b & 0x80 == 0 {
                            break;
                        }
                    }
                }
                0xB0 => {
                    if p + 1 >= ev.len() {
                        break;
                    }
                    let cc = ev[p];
                    let v = ev[p + 1];
                    p += 2;
                    if cc == 116 || cc == 117 {
                        loops.push((cc, v));
                    }
                }
                _ => break,
            },
        }
    }
    (loops, tempo)
}
