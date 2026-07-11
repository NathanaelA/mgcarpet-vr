//! Dump an MC2 level's objective stage board + the THINGs relevant to
//! each objective type (buildings for type-9, etc). Debug instrument
//! for the stage/objective engine.
//!
//! Usage: cargo run -p mgc-sim --example tmp_stages [baked/mc2/level-001.mgcl]
use std::path::Path;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "baked/mc2/level-001.mgcl".into());
    let file = std::fs::File::open(Path::new(&path)).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let things = &pkg.things.things;
    println!("== {path} ==  ({} things)", things.len());

    if let Some(s) = &pkg.stages {
        println!("objective rows:");
        for (i, c) in s.checkpoints.iter().enumerate() {
            let note = match c.index {
                0 => "collect mana",
                3 => "kill enemy player",
                5 => "release/fly-to point",
                7 => "kill creature (by model)",
                8 => "kill all players",
                9 => "destroy building",
                -1 => "unused",
                _ => "?",
            };
            println!(
                "  row {i}: type={} stage={} x={} y={}  [{note}]",
                c.index, c.stage, c.x, c.y
            );
            // Type 9 references table[stage].par1 = the building-type tag.
            if c.index == 9 {
                if let Some(t) = things.get(c.stage as usize) {
                    let tag = t.parent;
                    let n = things
                        .iter()
                        .filter(|t| t.class == 10 && t.model == 45 && t.parent == tag)
                        .count();
                    println!("        -> par1(tag)={tag}; {n} building(s) carry it");
                }
            }
        }
    }
    println!("buildings (class 10 model 45): idx x,y dis par1(tag)");
    for (i, t) in things.iter().enumerate() {
        if t.class == 10 && t.model == 45 {
            println!("  [{i}] {},{} dis={} par1={}", t.x, t.y, t.dis_id, t.parent);
        }
    }
}
