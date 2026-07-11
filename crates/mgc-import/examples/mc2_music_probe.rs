//! Throwaway probe: dump SOUND/MUSIC.DAT structure for MC2.
//! Usage: mc2_music_probe <gamedata-root>
//!
//! Parses per InitMusicBank_8EAD0 (Sound.cpp:5498):
//!   trailer u32 @ EOF-4 -> datapos
//!   @datapos: int16[4] driverarray (per-driver channel/song count)
//!   per channel: seek datapos+8 + (chan)*64, read type_v8[4] (16 bytes each):
//!       {dword_0 = header off, dword_4 = xmi-data off,
//!        sizeBytes_8 = header size, dword_12 = xmi-data size}
//!   at dword_4: xmi blob (maybe RNC "RNC\x01"); at dword_0: header (maybe RNC)
//!   header struct (216 B): stub[10], track_10[6] { xmiData(ptr,4B on disk as
//!     offset), stub_4[4], xmiSize_8(4B), word_12(2B), filename_14[18] } = 32B, stubb[14]

use std::path::Path;

fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn le_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

fn maybe_rnc(raw: &[u8]) -> Vec<u8> {
    if raw.len() >= 4 && &raw[0..3] == b"RNC" {
        match mgc_import::rnc::decompress(raw) {
            Ok(d) => {
                println!(
                    "      [RNC decompressed {} -> {} bytes]",
                    raw.len(),
                    d.len()
                );
                d
            }
            Err(e) => {
                println!("      [RNC decompress FAILED: {e:?}]");
                raw.to_vec()
            }
        }
    } else {
        raw.to_vec()
    }
}

fn hexdump(b: &[u8], n: usize) {
    let n = n.min(b.len());
    for chunk in b[..n].chunks(16) {
        let hex: String = chunk.iter().map(|x| format!("{x:02x} ")).collect();
        let asc: String = chunk
            .iter()
            .map(|&x| {
                if (0x20..0x7f).contains(&x) {
                    x as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("      {hex:<48} {asc}");
    }
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage: mc2_music_probe <gamedata-root>");
    let gd = mgc_import::gamedata::Gamedata::locate(Path::new(&root));
    let src = gd.mc2.expect("mc2 source");
    println!("source: {}", src.origin);

    let data = src.read("SOUND/MUSIC.DAT").expect("SOUND/MUSIC.DAT");
    println!("MUSIC.DAT total = {} bytes", data.len());

    let datapos = le_u32(&data, data.len() - 4) as usize;
    println!("trailer u32 @EOF-4 = datapos {datapos} (0x{datapos:x})");

    let driverarray = [
        le_i16(&data, datapos),
        le_i16(&data, datapos + 2),
        le_i16(&data, datapos + 4),
        le_i16(&data, datapos + 6),
    ];
    let names = ["G (GM/MPU)", "R (MT-32)", "F (FM/AdLib)", "W (AWE32)"];
    println!("driverarray int16[4] (song/channel count per driver):");
    for (i, c) in driverarray.iter().enumerate() {
        println!("  [{i}] {} = {c}", names[i]);
    }

    // The record table begins right after the 8-byte driverarray.
    let table = datapos + 8;
    // Number of channels/songs = max driver count.
    let nchan = *driverarray.iter().max().unwrap() as usize;

    for drv in 0..4 {
        let ndrv = driverarray[drv];
        if ndrv <= 0 {
            continue;
        }
        println!(
            "\n================ DRIVER {} : {} ================",
            drv, names[drv]
        );
        for chan in 0..(ndrv as usize) {
            let rec = table + chan * 64 + drv * 16;
            if rec + 16 > data.len() {
                println!("  chan {chan}: record OOB");
                continue;
            }
            let hdr_off = le_u32(&data, rec) as usize;
            let xmi_off = le_u32(&data, rec + 4) as usize;
            let hdr_size = le_u32(&data, rec + 8) as usize;
            let xmi_size = le_u32(&data, rec + 12) as usize;
            println!(
                "\n  -- song/chan {chan}: hdr@{hdr_off} size={hdr_size}  xmidata@{xmi_off} size={xmi_size}"
            );
            if hdr_off == 0xFFFF_FFFF || hdr_off + hdr_size > data.len() {
                println!("     (empty / OOB header)");
                continue;
            }
            // Header block (may be RNC).
            let hdr_raw = &data[hdr_off..hdr_off + hdr_size.min(data.len() - hdr_off)];
            let hdr = maybe_rnc(hdr_raw);
            // hdr layout: 8 byte prefix (byte_0..7) then sub1 (216): stub[10], 6x32B tracks, stubb[14]
            // The GetMusicSequenceCount counts tracks = sizeBytes_8 / sizeof(sub2type)=32
            // But actual per-track headers live at offset 8 + 10 = 18.
            let track_base = 8 + 10;
            let ntracks = (hdr_size / 32).min(6);
            println!(
                "     header prefix bytes: {:02x?}",
                &hdr[..8.min(hdr.len())]
            );
            println!("     (hdr_size/32 => up to {ntracks} track slots)");
            for t in 0..6 {
                let to = track_base + t * 32;
                if to + 32 > hdr.len() {
                    break;
                }
                let tk_data = le_u32(&hdr, to);
                let tk_size = le_u32(&hdr, to + 8);
                let tk_word12 = le_i16(&hdr, to + 12);
                let fname_bytes = &hdr[to + 14..to + 32];
                let fname: String = fname_bytes
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| c as char)
                    .collect();
                println!(
                    "       track[{t}] dataOff={tk_data} size={tk_size} word_12={tk_word12} name={fname:?}"
                );
            }

            // Now the actual XMI data blob (may be RNC).
            if xmi_off + 8 <= data.len() {
                let end = (xmi_off + xmi_size).min(data.len());
                let xmi_raw = &data[xmi_off..end];
                let xmi = maybe_rnc(xmi_raw);
                let magic4: String = xmi[..4.min(xmi.len())].iter().map(|&c| c as char).collect();
                println!("     xmi blob first 4 = {magic4:?} ; first 64 bytes:");
                hexdump(&xmi, 64);
                // Scan for IFF chunk ids.
                for id in [
                    "FORM", "XDIR", "INFO", "CAT ", "XMID", "TIMB", "RBRN", "EVNT",
                ] {
                    if let Some(p) = find(&xmi, id.as_bytes()) {
                        println!("       found {:?} @ {p}", id);
                    }
                }
                // How many EVNT (songs)?
                let ev = count(&xmi, b"EVNT");
                let forms = count(&xmi, b"FORM");
                println!("       #FORM={forms} #EVNT={ev}");
                // dump first ~48 bytes after first EVNT header (skip 8-byte chunk hdr).
                if let Some(p) = find(&xmi, b"EVNT") {
                    let start = p + 8;
                    println!("       first EVNT payload @ {start}, first 48 bytes:");
                    hexdump(&xmi[start.min(xmi.len())..], 48);
                }
            }
        }
    }
    let _ = nchan;
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}
