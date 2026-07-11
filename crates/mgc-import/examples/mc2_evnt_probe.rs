//! Deep-dump the first EVNT of the G-section bank-0 track (C2GAME1.GEN) to
//! hand-verify XMI event encoding (delta accumulation, note-on embedded VLQ dur).
use std::path::Path;
fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}
fn maybe_rnc(raw: &[u8]) -> Vec<u8> {
    if raw.len() >= 4 && &raw[0..3] == b"RNC" {
        mgc_import::rnc::decompress(raw).unwrap()
    } else {
        raw.to_vec()
    }
}
fn main() {
    let root = std::env::args().nth(1).unwrap();
    let gd = mgc_import::gamedata::Gamedata::locate(Path::new(&root));
    let src = gd.mc2.expect("mc2");
    let data = src.read("SOUND/MUSIC.DAT").unwrap();
    let datapos = le_u32(&data, data.len() - 4) as usize;
    let table = datapos + 8;
    // G driver = 0, channel/bank 0
    // G driver = 0, channel/bank 0 -> record slot at table + chan*64 + drv*16 = table.
    let rec = table;
    let xmi_off = le_u32(&data, rec + 4) as usize;
    let xmi_size = le_u32(&data, rec + 12) as usize;
    let xmi = maybe_rnc(&data[xmi_off..(xmi_off + xmi_size).min(data.len())]);
    let ev = find(&xmi, b"EVNT").unwrap();
    let len = le_u32be(&xmi, ev + 4);
    println!("first EVNT @ {ev}, chunk len (BE) = {len}");
    let mut p = ev + 8;
    let end = p + len as usize;
    let mut tokidx = 0;
    let mut abstime = 0i64;
    while p < end && tokidx < 60 {
        // accumulate delay bytes (<0x80)
        let mut delay = 0i64;
        while p < end && xmi[p] < 0x80 {
            delay += xmi[p] as i64;
            p += 1;
        }
        abstime += delay;
        if p >= end {
            break;
        }
        let st = xmi[p];
        p += 1;
        match st & 0xF0 {
            0xC0 | 0xD0 => {
                let d = xmi[p];
                p += 1;
                println!("t={abstime:5} (+{delay}) {st:02x} prog/chanp {d:02x}");
            }
            0x90 => {
                let note = xmi[p];
                let vel = xmi[p + 1];
                p += 2;
                // embedded duration VLQ (standard MIDI VLQ, continuation bit)
                let mut dur = 0u32;
                loop {
                    let b = xmi[p];
                    p += 1;
                    dur = (dur << 7) | (b & 0x7f) as u32;
                    if b & 0x80 == 0 {
                        break;
                    }
                }
                println!(
                    "t={abstime:5} (+{delay}) NOTE-ON ch{} note={note:02x} vel={vel:02x} dur={dur} (off@{})",
                    st & 0x0f,
                    abstime + dur as i64
                );
            }
            0x80 | 0xA0 | 0xB0 | 0xE0 => {
                let a = xmi[p];
                let b = xmi[p + 1];
                p += 2;
                let kind = match st & 0xf0 {
                    0x80 => "note-off",
                    0xA0 => "aftertouch",
                    0xB0 => "controller",
                    0xE0 => "pitchbend",
                    _ => "?",
                };
                println!(
                    "t={abstime:5} (+{delay}) {st:02x} {kind} ch{} {a:02x} {b:02x}",
                    st & 0x0f
                );
            }
            0xF0 => {
                if st == 0xFF {
                    let meta = xmi[p];
                    p += 1;
                    let l = xmi[p] as usize;
                    p += 1;
                    let body = &xmi[p..p + l];
                    p += l;
                    println!("t={abstime:5} (+{delay}) META {meta:02x} len={l} {body:02x?}");
                    if meta == 0x2f {
                        println!("  END");
                        break;
                    }
                } else {
                    // sysex
                    let mut l = 0u32;
                    loop {
                        let b = xmi[p];
                        p += 1;
                        l = (l << 7) | (b & 0x7f) as u32;
                        if b & 0x80 == 0 {
                            break;
                        }
                    }
                    p += l as usize;
                    println!("t={abstime:5} sysex {st:02x} len={l}");
                }
            }
            _ => {
                println!("t={abstime:5} UNKNOWN status {st:02x} @ {}", p - 1);
                break;
            }
        }
        tokidx += 1;
    }
}
fn le_u32be(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
