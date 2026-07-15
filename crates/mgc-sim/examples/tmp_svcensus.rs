//! Session-H census over all baked MC2 levels: the StageVar table
//! (slot-0 authorship, cadence+chain combos, kind-6 zero timers, which
//! creature models each var HOLDS — the m27/m9 special cases) plus the
//! objective-board stage==0 typed rows (the retail dead-guard set) and
//! the max authored disposition id (the 1..=64 storm-bound check).
//!
//! Usage: cargo run -p mgc-sim --example tmp_svcensus
use std::path::Path;

fn main() {
    let dir = Path::new("baked/mc2");
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("baked/mc2")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    names.sort();
    let mut max_dis_all = 0u16;
    let mut stage0 = Vec::new();
    for p in &names {
        let lvl = p.file_stem().unwrap().to_string_lossy().to_string();
        let file = std::fs::File::open(p).unwrap();
        let Ok(pkg) = mgc_formats::mgcl::read(file) else {
            continue;
        };
        let things = &pkg.things.things;
        // SLOT-indexed lookup (build_table's law — `pos != slot`, the
        // Session-E trace correction; array-position indexing lies).
        let by_slot: std::collections::HashMap<u32, (u16, u16)> = things
            .iter()
            .map(|t| (t.slot, (t.class, t.model)))
            .collect();
        let max_dis = things
            .iter()
            .map(|t| t.dis_id)
            .filter(|&d| d != 0xFFFF)
            .max()
            .unwrap_or(0);
        max_dis_all = max_dis_all.max(max_dis);
        let Some(s) = &pkg.stages else { continue };
        // Typed rows with stage==0 — retail's drop-guard is DEAD CODE
        // (binary-verified), so retail KEEPS these.
        for (i, c) in s.checkpoints.iter().enumerate() {
            if matches!(c.index, 1 | 2 | 4 | 6 | 7 | 9) && c.stage == 0 {
                stage0.push((lvl.clone(), i, c.index));
            }
        }
        // StageVar table.
        for (slot, v) in s.variables.iter().enumerate() {
            let byte0 = v.index as u8;
            let kind = byte0 & 0xF;
            if kind == 0 {
                continue;
            }
            let cadence = byte0 & 0x30;
            let chain = v.stage as u8;
            let hold_word = (v.x as u16) | ((v.y as u16) << 8);
            let hm = by_slot.get(&(hold_word as u32)).copied().unwrap_or((0, 0));
            let mut notes = String::new();
            if slot == 0 {
                notes.push_str(" SLOT0!");
            }
            if cadence != 0 && chain != 0 {
                notes.push_str(" CADENCE+CHAIN!");
            }
            if kind == 6 && v.data & 0xFFFF == 0 {
                notes.push_str(" K6-ZERO-TIMER!");
            }
            if hm == (5, 27) {
                notes.push_str(" HOLDS-M27!");
            }
            if hm == (5, 9) {
                notes.push_str(" HOLDS-M9!");
            }
            println!(
                "{lvl} slot={slot} kind={kind} byte0={byte0:#04x} chain={chain} \
                 hold={hold_word}(c{},m{}) data={:#x}{notes}",
                hm.0, hm.1, v.data
            );
        }
    }
    println!("\n== typed stage==0 rows (retail keeps, port drops) ==");
    for (l, row, ty) in &stage0 {
        println!("{l} row={row} type={ty}");
    }
    println!("\nmax authored dis_id (non-0xFFFF): {max_dis_all}");
}
