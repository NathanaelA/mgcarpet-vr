//! `blob-census` — WHICH BYTES OF THE RAW STRUCT IMAGE EVER MOVE.
//!
//! The `.mgcr` state channel is retail's master struct verbatim
//! (232,713 bytes on MC1), and every lane the harness has — graded or
//! shadowed — is a byte the DECODER lifts. A byte no decoder reads is
//! in no lane, so a divergence living there is invisible by
//! construction. This mode walks a take's records and counts, per byte
//! offset, how many records changed it against the previous record,
//! folding the two repeated layouts (the 8 × 2,049-byte wizard records
//! and the 1,000 × 164-byte pool records) onto their intra-record
//! offset, so each offset is reported once with a DECODED / UNDECODED
//! tag. Pool bytes are counted only on slots live in the new record
//! (`+64 != 0`); wizard bytes only on seated wizards (`+10 != 0`).
//!
//!     ./tools/conform blob-census recordings/<take>.mgcr [--limit <n>]
//!
//! Both games (the family is read off the header; the MC2 arm folds
//! the 8 × 2,124-byte player records and the 1,000 × 168-byte pool).
//! Output: one line per moving offset, most-moved first.

use mgc_formats::mgcr::{Family, Recording};

/// One game's image geometry + the decoder's coverage.
struct Layout {
    size: usize,
    wiz_base: usize,
    wiz_stride: usize,
    wiz_seated: fn(&[u8]) -> bool,
    wiz_decoded: fn(usize) -> bool,
    pool: usize,
    ent_stride: usize,
    ent_live: fn(&[u8]) -> bool,
    ent_decoded: fn(usize) -> bool,
    global_decoded: fn(usize) -> bool,
}

// ------------------------------------------------------------------ MC2

/// `decode_retail_ent_mc2` lifts `+0x04..+0x60` and `+0x62..+0xA8`.
fn ent_decoded_mc2(off: usize) -> bool {
    (0x04..0x60).contains(&off) || (0x62..0xA8).contains(&off)
}

/// `decode_retail_player_mc2`: the roster header, notify slot, name,
/// the six 26-wide book tables, the castle word, the hands and the
/// `type_str_164` flight block's decoded words (each with a 4-byte
/// tolerance — coverage, not a field map).
fn wiz_decoded_mc2(off: usize) -> bool {
    const T: usize = 998;
    const FLIGHT: [usize; 44] = [
        4, 6, 12, 14, 16, 30, 32, 36, 60, 62, 64, 92, 322, 326, 330, 332, 333, 334, 335, 340, 341,
        343, 345, 355, 397, 418, 420, 446, 449, 516, 518, 578, 580, 582, 584, 586, 609, 610, 1111,
        1112, 1116, 1117, 1118, 1119,
    ];
    if (0x9..0x14).contains(&off) || (0x1C..0x4F).contains(&off) || (0x39F..0x3E1).contains(&off) {
        return true;
    }
    if (0x649..0x837).contains(&off) || (0x846..0x84C).contains(&off) {
        return true;
    }
    if (1_080..1_082).contains(&off) || (2_103..2_107).contains(&off) {
        return true;
    }
    if off >= T {
        let r = off - T;
        if (871..923).contains(&r) {
            return true;
        }
        return FLIGHT.iter().any(|&f| r >= f && r < f + 4);
    }
    false
}

fn global_decoded_mc2(off: usize) -> bool {
    (8..16).contains(&off) || (0x31..0x35).contains(&off) || (0x36546..0x36548).contains(&off)
}

const MC2: Layout = Layout {
    size: 224_790,
    wiz_base: 0x2BDE,
    wiz_stride: 2_124,
    wiz_seated: |r| r[0xA] != 0 || r[0xB] != 0,
    wiz_decoded: wiz_decoded_mc2,
    pool: 0x6E8E,
    ent_stride: 168,
    ent_live: |r| r[0x3F] != 0,
    ent_decoded: ent_decoded_mc2,
    global_decoded: global_decoded_mc2,
};

// ------------------------------------------------------------------ MC1

const STRUCT: usize = 232_713;
const WIZARDS: usize = 13_323;
const WIZ_STRIDE: usize = 2_049;
const WIZ_COUNT: usize = 8;
const POOL: usize = 29_795;
const ENT_STRIDE: usize = 164;
const ENT_COUNT: usize = 1_000;

/// The entity offsets `decode_retail_ent_mc1` lifts (each lane's first
/// byte through its width).
fn ent_decoded(off: usize) -> bool {
    const RANGES: [(usize, usize); 5] = [
        (0, 60),    // chain_next, rand .. f59
        (61, 90),   // f61 .. frames89
        (90, 126),  // the six mailboxes
        (126, 134), // f126 .. f132
        (136, 164), // f136 .. owner_ptr
    ];
    RANGES.iter().any(|&(a, b)| off >= a && off < b)
}

/// The wizard offsets `decode_retail_wizard_mc1` lifts: the roster
/// header, the message slots, and the Type_160 block's decoded fields.
fn wiz_decoded(off: usize) -> bool {
    const T160: usize = 1_103;
    if off < 12 {
        return true;
    }
    // messages_13351_28: 8 × 68-byte slots (text + ticks + drawType).
    if (28..28 + 8 * 68).contains(&off) {
        return true;
    }
    if off < T160 {
        return false;
    }
    let r = off - T160;
    const RANGES: [(usize, usize); 30] = [
        (0, 8),     // move_bits, roll/pitch delta
        (12, 18),   // cmd_speed, v14, strafe
        (22, 30),   // knock, eff_pitch
        (46, 58),   // danger, castle, balloon_reg
        (84, 152),  // guard_reg
        (308, 312), // banked_houses
        (314, 322), // duel triple
        (326, 331), // charge, roll/pitch acc
        (331, 333), // grace
        (341, 343), // life_rate
        (343, 351), // shots, hits
        (359, 363), // kills
        (383, 387), // regen_stall
        (391, 394), // alerts
        (404, 408), // burst, poverty
        (415, 416), // ai_state
        (460, 524), // hate/war 8 × (2+2) interleaved
        (526, 530), // tempo, aggro
        (532, 628), // spell_list
        (628, 676), // learn
        (676, 724), // owned_slots
        (724, 772), // cooldown
        (916, 940), // blue
        (940, 942), // hand_left
        (944, 946), // hand_right
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
    ];
    RANGES.iter().any(|&(a, b)| b > a && r >= a && r < b)
}

/// The global (non-repeated) offsets the decoder lifts.
fn global_decoded(off: usize) -> bool {
    (4..12).contains(&off)          // rand, local player, count
        || (12..32).contains(&off)  // spawn_count
        || (36..44).contains(&off)  // erupting, plume, free top
        || (593..4_593).contains(&off) // free stack cells
        || (4_593..8_597).contains(&off) // recycle top + cells
        || (232_707..232_709).contains(&off) // level
}

const MC1: Layout = Layout {
    size: STRUCT,
    wiz_base: WIZARDS,
    wiz_stride: WIZ_STRIDE,
    wiz_seated: |r| r[10] != 0 || r[11] != 0,
    wiz_decoded,
    pool: POOL,
    ent_stride: ENT_STRIDE,
    ent_live: |r| r[64] != 0,
    ent_decoded,
    global_decoded,
};

pub(crate) fn blob_census(path: &std::path::Path, limit: Option<usize>) -> i32 {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => {
            println!("BLOB {name}: ERROR — {e}");
            return 2;
        }
    };
    let lay: &Layout = match rec.header.family() {
        Ok(Family::Mc1) => &MC1,
        Ok(Family::Mc2) => &MC2,
        Err(e) => {
            println!("BLOB {name}: ERROR — {e}");
            return 2;
        }
    };
    let (wiz_end, pool_end) = (
        lay.wiz_base + lay.wiz_stride * WIZ_COUNT,
        lay.pool + lay.ent_stride * ENT_COUNT,
    );
    let mut ent = vec![0u64; lay.ent_stride];
    let mut wiz = vec![0u64; lay.wiz_stride];
    let mut glob = vec![0u64; lay.size];
    let mut prev: Option<Vec<u8>> = None;
    let mut records = 0usize;
    while let Some(r) = rec.next_tick() {
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                println!("BLOB {name}: ERROR — {e}");
                return 2;
            }
        };
        let Some(cur) = r.state else { continue };
        if cur.len() != lay.size {
            println!(
                "BLOB {name}: ERROR — struct image {} bytes, want {}",
                cur.len(),
                lay.size
            );
            return 2;
        }
        if let Some(p) = &prev {
            // Pool: live slots only.
            for s in 0..ENT_COUNT {
                let o = lay.pool + s * lay.ent_stride;
                if !(lay.ent_live)(&cur[o..o + lay.ent_stride]) {
                    continue;
                }
                for k in 0..lay.ent_stride {
                    if cur[o + k] != p[o + k] {
                        ent[k] += 1;
                    }
                }
            }
            // Wizards: seated only.
            for w in 0..WIZ_COUNT {
                let o = lay.wiz_base + w * lay.wiz_stride;
                if !(lay.wiz_seated)(&cur[o..o + lay.wiz_stride]) {
                    continue;
                }
                for k in 0..lay.wiz_stride {
                    if cur[o + k] != p[o + k] {
                        wiz[k] += 1;
                    }
                }
            }
            // Everything else, by absolute offset.
            for (k, g) in glob.iter_mut().enumerate() {
                if (lay.wiz_base..wiz_end).contains(&k) || (lay.pool..pool_end).contains(&k) {
                    continue;
                }
                if cur[k] != p[k] {
                    *g += 1;
                }
            }
        }
        prev = Some(cur);
        records += 1;
        if limit.is_some_and(|n| records >= n) {
            break;
        }
    }
    println!("== blob-census {name}: {records} record(s) with state");
    let show = |title: &str, v: &[u64], decoded: &dyn Fn(usize) -> bool| {
        let mut rows: Vec<(usize, u64)> = v
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(k, c)| (k, *c))
            .collect();
        let undec = rows.iter().filter(|(k, _)| !decoded(*k)).count();
        println!(
            "  -- {title}: {} moving offset(s), {undec} UNDECODED",
            rows.len()
        );
        rows.sort_by_key(|&(k, c)| (std::cmp::Reverse(c), k));
        for (k, c) in rows {
            println!(
                "     {}{k:>7} changed {c:>9}×",
                if decoded(k) { "        " } else { "UNDECODED" }
            );
        }
    };
    show(
        &format!(
            "POOL RECORD (offset within the {}-byte record, live slots)",
            lay.ent_stride
        ),
        &ent,
        &lay.ent_decoded,
    );
    show(
        &format!(
            "WIZARD RECORD (offset within the {}-byte record, seated)",
            lay.wiz_stride
        ),
        &wiz,
        &lay.wiz_decoded,
    );
    show(
        "GLOBALS (absolute struct offset, outside both arrays)",
        &glob,
        &lay.global_decoded,
    );
    0
}

/// `blob-census --init` — WHAT FRAME 1 WROTE: the take's INIT RECORD
/// (pre-frame-1) against record 0, byte by byte. The pool is read on
/// the records BOTH images hold (a birth is the constructor's, and
/// `lane-check` counts those); wizard records on every row either
/// image seats; globals by absolute offset, old → new.
pub(crate) fn blob_frame1(path: &std::path::Path) -> i32 {
    let name = crate::verify::take_stem(path);
    let fail = |e: String| {
        println!("FRAME1 {name}: ERROR — {e}");
        2
    };
    let mut rec = match Recording::open(path) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let lay: &Layout = match rec.header.family() {
        Ok(Family::Mc1) => &MC1,
        Ok(Family::Mc2) => &MC2,
        Err(e) => return fail(e),
    };
    let Some(init) = rec.init.clone().and_then(|i| i.tick.state) else {
        println!("FRAME1 {name}: NONE — no init record");
        return 0;
    };
    let (cur, t0) = loop {
        match rec.next_tick() {
            Some(Ok(r)) => {
                if let Some(s) = r.state {
                    break (s, r.t);
                }
            }
            Some(Err(e)) => return fail(e),
            None => return fail("no record with a state".into()),
        }
    };
    if cur.len() != lay.size || init.len() != lay.size {
        return fail(format!(
            "struct images {} / {} bytes, want {}",
            init.len(),
            cur.len(),
            lay.size
        ));
    }
    let (wiz_end, pool_end) = (
        lay.wiz_base + lay.wiz_stride * WIZ_COUNT,
        lay.pool + lay.ent_stride * ENT_COUNT,
    );
    let (class_at, model_at) = if lay.ent_stride == 164 {
        (64, 65)
    } else {
        (0x3F, 0x40)
    };
    println!("== blob-census --init {name}: init record → record 0 (t={t0})");
    // Pool: records both images hold.
    let mut ent: Vec<(u64, Vec<String>)> = vec![(0, Vec::new()); lay.ent_stride];
    let mut both = 0usize;
    for s in 1..ENT_COUNT {
        let o = lay.pool + s * lay.ent_stride;
        let (a, b) = (&init[o..o + lay.ent_stride], &cur[o..o + lay.ent_stride]);
        if !(lay.ent_live)(a) || !(lay.ent_live)(b) {
            continue;
        }
        both += 1;
        for k in 0..lay.ent_stride {
            if a[k] != b[k] {
                ent[k].0 += 1;
                if ent[k].1.len() < 3 {
                    ent[k].1.push(format!(
                        "slot {s} ({},{}) {:#04x}→{:#04x}",
                        b[class_at], b[model_at], a[k], b[k]
                    ));
                }
            }
        }
    }
    let (mut rows, mut undec) = (0, 0);
    println!("  -- POOL ({both} record(s) held by both images):");
    for (k, (c, eg)) in ent.iter().enumerate() {
        if *c == 0 {
            continue;
        }
        rows += 1;
        let d = (lay.ent_decoded)(k);
        if !d {
            undec += 1;
        }
        println!(
            "     {}+{k:<4} {c:>5} record(s)  {}",
            if d { "         " } else { "UNDECODED" },
            eg.join(" · ")
        );
    }
    let (pool_rows, pool_undec) = (rows, undec);
    // Wizards.
    let (mut wrows, mut wundec) = (0, 0);
    for w in 0..WIZ_COUNT {
        let o = lay.wiz_base + w * lay.wiz_stride;
        let (a, b) = (&init[o..o + lay.wiz_stride], &cur[o..o + lay.wiz_stride]);
        if !(lay.wiz_seated)(a) && !(lay.wiz_seated)(b) {
            continue;
        }
        let ch: Vec<usize> = (0..lay.wiz_stride).filter(|&k| a[k] != b[k]).collect();
        let un: Vec<String> = ch
            .iter()
            .filter(|&&k| !(lay.wiz_decoded)(k))
            .map(|&k| format!("+{k} {:#04x}→{:#04x}", a[k], b[k]))
            .collect();
        wrows += ch.len();
        wundec += un.len();
        println!(
            "  -- WIZARD {w}: {} byte(s) changed, {} UNDECODED{}",
            ch.len(),
            un.len(),
            if un.is_empty() {
                String::new()
            } else {
                format!(": {}", un.join(" "))
            }
        );
    }
    // Globals.
    let (mut grows, mut gundec) = (0, 0);
    let mut line = Vec::new();
    for k in 0..lay.size {
        if (lay.wiz_base..wiz_end).contains(&k) || (lay.pool..pool_end).contains(&k) {
            continue;
        }
        if init[k] != cur[k] {
            grows += 1;
            let d = (lay.global_decoded)(k);
            if !d {
                gundec += 1;
                if line.len() < 96 {
                    line.push(format!("{k:#x} {:#04x}→{:#04x}", init[k], cur[k]));
                }
            }
        }
    }
    println!(
        "  -- GLOBALS: {grows} byte(s) changed, {gundec} UNDECODED{}",
        if line.is_empty() {
            String::new()
        } else {
            format!(": {}", line.join(" "))
        }
    );
    println!(
        "FRAME1 {name}: pool offsets {pool_rows} ({pool_undec} undecoded) on {both} shared \
         record(s) · wizard bytes {wrows} ({wundec} undecoded) · global bytes {grows} ({gundec} \
         undecoded)"
    );
    0
}
