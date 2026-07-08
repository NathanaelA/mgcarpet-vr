//! Campaign-derived spell inference for the `plausible_spellbook`
//! playtest instrument.
//!
//! MC1's spellbook is cross-level state: you begin with nothing and
//! collect spells one red jar at a time; a spell you never picked up you
//! simply don't have (disassembly-verified — see docs/ROADMAP.md "Campaign
//! spell progression"). That makes an individual level un-playtestable in
//! isolation, because the faithful spellbook depends on everything
//! collected in the levels before it.
//!
//! This module reconstructs an UPPER BOUND on that state purely from the
//! baked level data. A spell jar is a static class-12 placement record
//! (`Thing`), and its `model` is the spell id — the same field the engine
//! reads when granting the spell (remc1 sub_main.cpp:64794 / :55318), and
//! the same field its own load-time jar census indexes (:43952-55). So for
//! a target level N we union the jar spell-ids of every campaign level
//! BEFORE N (`0..N`, excluding N itself — the jars in N are what you are
//! meant to collect while playing it) and grant that set at level start.
//!
//! It is a maximum, not a real playthrough: a diligent player COULD have
//! every spell in the union, which is exactly what a playtest wants. It
//! omits enemy-wizard-corpse spells (the other legitimate source) — fine
//! for the non-wizard levels this instrument targets; see the ROADMAP note.

use std::path::Path;

use mgc_formats::{Game, LevelPackage, ThingKind, Things, mgcl};
use mgc_sim::spells::SPELL_COUNT;

/// The MC1 campaign is the reachable levels (indices from 0 up), MINUS a
/// handful that the shipped game never routes to: five indices blacklisted
/// from the release campaign before 49, and the "lost levels" past 49
/// (multiplayer maps and experiments — valid, parseable records that normal
/// play never reaches). Both are the same case: not on the campaign path,
/// so their jars were never collectable, so they must not contribute to the
/// union. One blacklist covers both — no separate upper bound is needed,
/// since the union for a target level N only ever scans `0..N` and a
/// legitimate campaign target is itself below the lost-level range.
///
/// The five pre-49 skips are `{8, 17, 28, 33, 39}` — the single-player
/// campaign hardcodes bumping its level counter past them (remc1
/// `sub_34070`, sub_main.cpp:41456; player-verified for 008/017, see
/// docs/ROADMAP.md "MC1 CAMPAIGN SKIP TABLE"). 008/017/028/033 are
/// complete worlds parked in the campaign index range (multiplayer maps);
/// 039 is the authentically-broken flat-plateau level. Campaign = indices
/// 0-49 minus these five = 45 played levels; 50-69 are the multiplayer
/// pool, excluded by the `< 50` gate in [`is_campaign_level`].
const MC1_BLACKLIST: &[u32] = &[8, 17, 28, 33, 39];

/// The class-12 jar placement class (a spell pickup). Its `model` is the
/// granted spell id (0..24).
const JAR_CLASS: u16 = 12;

/// Is `level` a reachable MC1 campaign level (not blacklisted, not a lost
/// level)? Lost levels 50+ are excluded so that launching the instrument on
/// one never treats other lost levels as collectable sources.
fn is_campaign_level(level: u32) -> bool {
    level < 50 && !MC1_BLACKLIST.contains(&level)
}

/// The spell ids of every jar placed in one level's records. Counts jars
/// behind trigger dispositions too — a triggered jar is still collectable
/// in a completed playthrough, so it belongs in the "could have" set.
fn jar_spells_in(things: &Things) -> Vec<u8> {
    let mut out = Vec::new();
    for t in &things.things {
        if t.kind == ThingKind::Entity
            && t.class == JAR_CLASS
            && (t.model as usize) < SPELL_COUNT
        {
            let s = t.model as u8;
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// Result of a plausible-spellbook computation: the spell-id set to grant,
/// plus which campaign levels were actually scanned (for an honest log —
/// never a silent claim of completeness).
pub struct Plausible {
    pub spells: Vec<u8>,
    pub scanned_levels: Vec<u32>,
    pub skipped_levels: Vec<u32>,
}

/// Compute the plausible spellbook for `target_level`, reading sibling
/// `level-NNN.mgcl` files from `level_dir` (the directory the running level
/// was loaded from). Unions the jars of every campaign level in `0..N`
/// (excluding N). Non-MC1 packages return an empty set (the instrument is
/// MC1-only — MC2 spell handling is a separate system).
pub fn plausible_spellbook(level_dir: &Path, package: &LevelPackage) -> Plausible {
    let mut spells: Vec<u8> = Vec::new();
    let mut scanned = Vec::new();
    let mut skipped = Vec::new();

    if package.meta.game != Game::MagicCarpet1 {
        return Plausible {
            spells,
            scanned_levels: scanned,
            skipped_levels: skipped,
        };
    }

    let target = package.meta.level;
    for n in 0..target {
        if !is_campaign_level(n) {
            continue;
        }
        let path = level_dir.join(format!("level-{n:03}.mgcl"));
        let Ok(file) = std::fs::File::open(&path) else {
            skipped.push(n);
            continue;
        };
        let Ok(pkg) = mgcl::read(file) else {
            skipped.push(n);
            continue;
        };
        for s in jar_spells_in(&pkg.things) {
            if !spells.contains(&s) {
                spells.push(s);
            }
        }
        scanned.push(n);
    }
    spells.sort_unstable();
    Plausible {
        spells,
        scanned_levels: scanned,
        skipped_levels: skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mgc_formats::Thing;

    fn thing(kind: ThingKind, class: u16, model: u16) -> Thing {
        Thing {
            slot: 0,
            kind,
            class,
            model,
            x: 0,
            y: 0,
            dis_id: 0,
            swi_sz: 0,
            swi_id: 0,
            parent: 0,
            child: 0,
            par3: None,
        }
    }

    #[test]
    fn jar_census_picks_class12_entities_only_and_dedups() {
        let things = Things {
            things: vec![
                thing(ThingKind::Entity, 12, 0),  // Fireball jar
                thing(ThingKind::Entity, 12, 3),  // Possess jar
                thing(ThingKind::Entity, 12, 0),  // dup Fireball — ignored
                thing(ThingKind::Entity, 5, 12),  // a villager, NOT a jar
                thing(ThingKind::Marker, 12, 7),  // a marker, not a placed jar
                thing(ThingKind::Entity, 12, 99), // out-of-range model — ignored
            ],
        };
        let mut got = jar_spells_in(&things);
        got.sort_unstable();
        assert_eq!(got, vec![0, 3]);
    }

    #[test]
    fn campaign_membership_excludes_blacklist_and_lost_levels() {
        assert!(is_campaign_level(0));
        assert!(is_campaign_level(49));
        assert!(!is_campaign_level(50), "lost levels 50+ are not campaign");
        assert!(!is_campaign_level(120));
        for &b in MC1_BLACKLIST {
            assert!(!is_campaign_level(b), "blacklisted {b} is not campaign");
        }
    }
}
