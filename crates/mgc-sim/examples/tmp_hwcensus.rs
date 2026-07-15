//! Hidden-Worlds reachability census (docs/SURVEY-MC1HW.md §7). Scan the
//! 73 baked `mc1hw` levels to separate REAL delta work from theoretical:
//!   1. spell 20 availability  — wizard `starting_spells[20]`/`allowed_spells[20]`
//!   2. `(10,53)` THING         — the HW mana-drain entity (§4 content gap)
//!   3. new-homing models 16/18/19 as authored THINGS (any class)
//!   4. reached TMAPS `sprite_base` — flags 153/156 (corrupt arctic) + 76/177
//! Mirrors the runtime spawn gate: entity records only, marker rows
//! (x|y >= 256) never spawn.
use mgc_formats::{LevelPackage, ThingKind};
use mgc_sim::ids::GameId;
use mgc_sim::mc1::entities::{Mc1TypePick, mc1_entity_parts, mc1_entity_type};
use mgc_sim::mc1::sprite_stats::SPRITE_STATS;
use std::collections::{BTreeMap, BTreeSet};

const SPELL20: usize = 20;

/// Every type_index a (class, model) can resolve to (over-approximate:
/// take BOTH branches of every random pick, plus multipart segments).
fn candidate_types(class: u16, model: u16) -> Vec<u16> {
    let mut v = Vec::new();
    if let Some(pick) = mc1_entity_type(class, model) {
        match pick {
            Mc1TypePick::Const(i) => v.push(i),
            Mc1TypePick::RandomBit(a, b)
            | Mc1TypePick::RandomSevenSplit(a, b)
            | Mc1TypePick::AlternateByCount(a, b) => {
                v.push(a);
                v.push(b);
            }
            Mc1TypePick::Mana => {
                v.push(77);
                v.push(280);
            }
        }
    }
    v.extend_from_slice(mc1_entity_parts(class, model));
    v
}

fn main() {
    let arg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "baked/mc1hw".into());
    let root = std::path::Path::new(&arg);
    let mut names: Vec<_> = std::fs::read_dir(root)
        .expect("baked/mc1hw missing — bake mc1hw first")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mgcl"))
        .collect();
    names.sort();

    let mut levels = 0u32;
    let mut total_records = 0u64;
    let mut known_records = 0u64;

    // spell 20
    let mut spell20_grant: BTreeSet<String> = BTreeSet::new(); // any wizard granted
    let mut spell20_allow: BTreeSet<String> = BTreeSet::new(); // any wizard allowed
    // (10,53) + homing models
    let mut thing1053: BTreeSet<String> = BTreeSet::new();
    let mut homing_models: BTreeMap<(u16, u16), (u32, BTreeSet<String>)> = BTreeMap::new();
    // full THING tally + reached sprite_base
    let mut unknown: BTreeMap<(u16, u16), (u32, BTreeSet<String>)> = BTreeMap::new();
    let mut sprite_base: BTreeMap<u16, (u32, BTreeSet<String>)> = BTreeMap::new();

    for path in names {
        let file = std::fs::File::open(&path).unwrap();
        let pkg: LevelPackage = match mgc_formats::mgcl::read(file) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                continue;
            }
        };
        levels += 1;
        let lvl = path.file_stem().unwrap().to_string_lossy().into_owned();

        // spell 20 availability
        if let Some(w) = &pkg.wizards {
            for wc in &w.wizards {
                if wc.starting_spells.get(SPELL20).is_some_and(|&f| f != 0) {
                    spell20_grant.insert(lvl.clone());
                }
                if wc
                    .allowed_spells
                    .as_ref()
                    .and_then(|a| a.get(SPELL20))
                    .is_some_and(|&f| f != 0)
                {
                    spell20_allow.insert(lvl.clone());
                }
            }
        }

        for t in &pkg.things.things {
            if t.kind != ThingKind::Entity || t.x >= 256 || t.y >= 256 {
                continue;
            }
            if t.class == 0 {
                continue;
            }
            total_records += 1;
            if GameId::Mc1Hw.known_thing(t.class, t.model) {
                known_records += 1;
            } else {
                let e = unknown.entry((t.class, t.model)).or_default();
                e.0 += 1;
                e.1.insert(lvl.clone());
            }

            if (t.class, t.model) == (10, 53) {
                thing1053.insert(lvl.clone());
            }
            if matches!(t.model, 16 | 18 | 19) {
                let e = homing_models.entry((t.class, t.model)).or_default();
                e.0 += 1;
                e.1.insert(lvl.clone());
            }

            for ti in candidate_types(t.class, t.model) {
                if let Some(s) = SPRITE_STATS.get(ti as usize) {
                    let e = sprite_base.entry(s.sprite_base).or_default();
                    e.0 += 1;
                    e.1.insert(lvl.clone());
                }
            }
        }
    }

    println!("=== Hidden Worlds reachability census ({levels} levels) ===\n");
    println!(
        "THING records: {total_records} spawnable, {known_records} admitted ({:.1}%)\n",
        100.0 * known_records as f64 / total_records.max(1) as f64
    );

    println!("--- 1. spell 20 (Fire Storm) availability ---");
    println!(
        "  granted to a wizard in {} levels{}",
        spell20_grant.len(),
        example(&spell20_grant)
    );
    println!(
        "  allowed (mask) in       {} levels{}\n",
        spell20_allow.len(),
        example(&spell20_allow)
    );

    println!("--- 2. (10,53) HW mana-drain THING ---");
    if thing1053.is_empty() {
        println!("  NONE authored in any level -> §4 THING-registry work NOT needed\n");
    } else {
        println!(
            "  present in {} levels{} -> register + arm (10,53)\n",
            thing1053.len(),
            example(&thing1053)
        );
    }

    println!("--- 3. authored THINGS with new-homing models 16/18/19 ---");
    if homing_models.is_empty() {
        println!("  none placed as THINGS (expected: models are cast-time children)\n");
    } else {
        for ((c, m), (n, lv)) in &homing_models {
            println!("  ({c},{m}) x{n} / {} levels{}", lv.len(), example(lv));
        }
        println!();
    }

    println!("--- 4. reached TMAPS sprite_base (load-placed billboards) ---");
    let flags = [76u16, 153, 156, 177];
    for f in flags {
        match sprite_base.get(&f) {
            Some((n, lv)) => println!(
                "  base {f:3}: REACHED x{n} / {} levels{}",
                lv.len(),
                example(lv)
            ),
            None => println!("  base {f:3}: never reached as a base"),
        }
    }
    let max_base = sprite_base.keys().max().copied().unwrap_or(0);
    let near: Vec<u16> = sprite_base
        .keys()
        .filter(|&&b| (140..=160).contains(&b))
        .copied()
        .collect();
    println!("  (max reached base = {max_base}; bases in 140..=160: {near:?})\n");

    println!("--- misfit tally (records / levels / e.g.) ---");
    if unknown.is_empty() {
        println!("  none — registry admits 100% of authored HW records");
    } else {
        let mut rows: Vec<_> = unknown.into_iter().collect();
        rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        for ((c, m), (n, lv)) in rows {
            println!(
                "  ({c:2},{m:3}) x{n:<4} / {} levels{}",
                lv.len(),
                example(&lv)
            );
        }
    }
}

fn example(set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        return String::new();
    }
    let ex: Vec<_> = set.iter().take(4).cloned().collect();
    format!("  e.g. {}", ex.join(", "))
}
