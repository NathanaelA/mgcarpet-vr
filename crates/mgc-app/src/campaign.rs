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
use mgc_sim::mc1::spells::SPELL_COUNT;

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
        if t.kind == ThingKind::Entity && t.class == JAR_CLASS && (t.model as usize) < SPELL_COUNT {
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
    /// Spells the jar union held but the TARGET level's availability
    /// mask strips (logged by the caller, never silently dropped).
    pub masked: Vec<u8>,
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
            masked: Vec::new(),
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
    let masked = apply_level_mask(&mut spells, package);
    Plausible {
        spells,
        scanned_levels: scanned,
        skipped_levels: skipped,
        masked,
    }
}

/// The original's per-level grant is (availability mask) AND
/// (collected flag) — sub_main.cpp :49218-41: the human receives
/// spell v14 iff `var_230983[v14] == 1` besides having collected it.
/// The mask is the level tail's HUMAN slot-0 `allowed_spells` (all-1
/// through campaign index 024; SELECTIVE from 025 on — the regime
/// that strips your book at level start so spells are rediscovered
/// in play). The jar union models the collected flags, so it must
/// intersect the target level's mask exactly like retail. Returns
/// the stripped ids; a maskless package (MC2, old bake) is a no-op.
fn apply_level_mask(spells: &mut Vec<u8>, package: &LevelPackage) -> Vec<u8> {
    let Some(mask) = package
        .wizards
        .as_ref()
        .and_then(|w| w.wizards.first())
        .and_then(|h| h.allowed_spells.as_ref())
    else {
        return Vec::new();
    };
    let mut masked = Vec::new();
    spells.retain(|&s| {
        let ok = mask.get(s as usize).is_none_or(|&v| v == 1);
        if !ok {
            masked.push(s);
        }
        ok
    });
    masked
}

// ---------------------------------------------------------------------------
// MC2 arm — a plausible spellbook that carries spell EXPERIENCE, not just a
// learned set (MC2's book is XP-driven: each spell has a per-tier `xpos1`
// ladder). Unlike MC1 there is NO campaign-progression / secret-branch table
// anywhere in the game data — levels are addressed purely by archive index and
// the only hint that any level is "secret" is a speech-row comment. So a
// plausible passthrough for MC2 is, unavoidably, ARCHIVE-INDEX ORDER
// (`0..target`), the same assumption MC1 makes — flagged in the log so the
// estimate is never mistaken for a verified route. It is an UPPER BOUND: the
// learned set unions every collectable spell jar, and the XP assumes the
// player owned every spell when every scroll was collected (the "found every
// secret, maxed everything" ceiling the playtest wants).

/// The MC2 spell jar class (class-15 token; `model` = spell id 0..25).
const JAR_CLASS_MC2: u16 = 15;
/// The MC2 XP scroll: class 14, model 5 (`UpdateScroll_59C80`, tick state 10).
const SCROLL_CLASS_MC2: u16 = 14;
const SCROLL_MODEL_MC2: u16 = 5;
/// The 26-spell MC2 book width.
const MC2_SPELL_COUNT: usize = 26;
/// Single-player scroll XP: `UpdateExperience_6E090` grants this to EVERY
/// owned spell per scroll (not per-spell-targeted).
const MC2_SCROLL_XP: i32 = 4;
/// Fireball(0) + Possess(1): `mc2_seed_default_spells` grants these at every
/// MC2 level start, so they are always in the "could have" learned set.
const MC2_SEED_SPELLS: [u8; 2] = [0, 1];

/// A plausible MC2 spellbook: per learned spell, a plausible BANKED XP (the
/// sim derives the tier from each spell's `xpos1` ladder). Plus the census
/// provenance for an honest log.
pub struct PlausibleMc2 {
    /// `(spell_id, banked_xp)` for each spell plausibly learned by this point.
    pub grants: Vec<(u8, i32)>,
    pub scanned_levels: Vec<u32>,
    pub skipped_levels: Vec<u32>,
    /// Total XP scrolls acquirable in the scanned levels (the per-spell
    /// banked XP = `2 × scroll_count × MC2_SCROLL_XP`, the debug heuristic).
    pub scroll_count: u32,
}

/// The class-15 spell ids and class-14 scroll count placed in one level's
/// records. Counts latent (disposition/stage-gated) records too — they are
/// still class-15/14 THINGs in the table, collectable in a full playthrough
/// (same rationale as [`jar_spells_in`]).
fn mc2_jars_and_scrolls(things: &Things) -> (Vec<u8>, u32) {
    let mut jars = Vec::new();
    let mut scrolls = 0u32;
    for t in &things.things {
        if t.kind != ThingKind::Entity {
            continue;
        }
        if t.class == JAR_CLASS_MC2 && (t.model as usize) < MC2_SPELL_COUNT {
            let s = t.model as u8;
            if !jars.contains(&s) {
                jars.push(s);
            }
        } else if t.class == SCROLL_CLASS_MC2 && t.model == SCROLL_MODEL_MC2 {
            scrolls += 1;
        }
    }
    (jars, scrolls)
}

/// Compute the plausible MC2 spellbook for `target_level` by scanning sibling
/// `level-NNN.mgcl` files in archive-index order `0..target` (see the module
/// note on the missing ordering data). Non-MC2 packages return empty.
pub fn plausible_spellbook_mc2(level_dir: &Path, package: &LevelPackage) -> PlausibleMc2 {
    let mut learned: Vec<u8> = MC2_SEED_SPELLS.to_vec();
    let mut scroll_count = 0u32;
    let mut scanned = Vec::new();
    let mut skipped = Vec::new();

    if package.meta.game != Game::MagicCarpet2 {
        return PlausibleMc2 {
            grants: Vec::new(),
            scanned_levels: scanned,
            skipped_levels: skipped,
            scroll_count,
        };
    }

    for n in 0..package.meta.level {
        let path = level_dir.join(format!("level-{n:03}.mgcl"));
        let Ok(file) = std::fs::File::open(&path) else {
            skipped.push(n);
            continue;
        };
        let Ok(pkg) = mgcl::read(file) else {
            skipped.push(n);
            continue;
        };
        let (jars, scrolls) = mc2_jars_and_scrolls(&pkg.things);
        for s in jars {
            if !learned.contains(&s) {
                learned.push(s);
            }
        }
        scroll_count += scrolls;
        scanned.push(n);
    }
    learned.sort_unstable();
    // Each scroll grants MC2_SCROLL_XP to every owned spell. As a DEBUG
    // heuristic we count 2× the collectable scrolls: usage-based XP (which
    // we can't simulate) is a real second source, and testing later levels
    // showed the scroll-only floor lands too low. The sim's `mc2_relevel`
    // still clamps to each spell's tier ladder, so over-shooting is safe.
    let xp = MC2_SCROLL_XP * (2 * scroll_count) as i32;
    let grants = learned.into_iter().map(|s| (s, xp)).collect();
    PlausibleMc2 {
        grants,
        scanned_levels: scanned,
        skipped_levels: skipped,
        scroll_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mgc_formats::Thing;
    use mgc_formats::{BAKE_EPOCH, FORMAT_VERSION, Importer, Meta, WizardConfig, Wizards};

    /// The target level's availability mask (human slot-0
    /// allowed_spells, retail :49229) intersects the jar union — the
    /// mask regime past campaign index 024; maskless packages no-op.
    #[test]
    fn level_mask_strips_unavailable_spells() {
        let mut allowed = vec![1u8; 24];
        allowed[15] = 0; // this level withholds Lightning Bolt
        allowed[16] = 0; // ...and Create Castle
        let package = LevelPackage {
            meta: Meta {
                format_version: FORMAT_VERSION,
                bake_epoch: BAKE_EPOCH,
                game: Game::MagicCarpet1,
                level: 30,
                source: None,
                importer: Importer {
                    name: "test".into(),
                    version: "0".into(),
                },
            },
            things: Things { things: Vec::new() },
            gen_params: None,
            header: None,
            wizards: Some(Wizards {
                wizards: vec![WizardConfig {
                    aggression: 0,
                    reflexes: None,
                    perception: None,
                    life: None,
                    starting_spells: vec![0; 24],
                    starting_spell_levels: Vec::new(),
                    blocked_spells: Vec::new(),
                    accuracy: None,
                    tempo: None,
                    castle_level: None,
                    allowed_spells: Some(allowed),
                }],
                player_count: Some(1),
                tail_38800: None,
            }),
            stages: None,
            terrain: None,
        };
        let mut spells = vec![0u8, 3, 15, 16, 20];
        let masked = apply_level_mask(&mut spells, &package);
        assert_eq!(spells, vec![0, 3, 20], "mask strips 15/16, keeps the rest");
        assert_eq!(masked, vec![15, 16], "strips reported for the log");
        // No mask data (MC2 / old bake) = a no-op, nothing stripped.
        let bare = LevelPackage {
            wizards: None,
            ..package
        };
        let mut spells = vec![15u8, 16];
        assert!(apply_level_mask(&mut spells, &bare).is_empty());
        assert_eq!(spells, vec![15, 16]);
    }

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
    fn mc2_census_unions_class15_jars_and_counts_class14_scrolls() {
        let things = Things {
            things: vec![
                thing(ThingKind::Entity, 15, 4),  // spell-4 jar
                thing(ThingKind::Entity, 15, 9),  // spell-9 jar
                thing(ThingKind::Entity, 15, 4),  // dup — ignored
                thing(ThingKind::Entity, 14, 5),  // XP scroll
                thing(ThingKind::Entity, 14, 5),  // XP scroll
                thing(ThingKind::Entity, 14, 1),  // a RISER (class-14 model 1), NOT a scroll
                thing(ThingKind::Entity, 14, 0),  // class-14 model 0, NOT a scroll
                thing(ThingKind::Entity, 12, 0),  // an MC1 jar class — ignored in MC2
                thing(ThingKind::Marker, 15, 3),  // marker, not a placed jar
                thing(ThingKind::Entity, 15, 99), // out-of-range spell — ignored
            ],
        };
        let (mut jars, scrolls) = mc2_jars_and_scrolls(&things);
        jars.sort_unstable();
        assert_eq!(jars, vec![4, 9], "class-15 jars deduped by spell model");
        assert_eq!(scrolls, 2, "only class-14 model-5 counts as an XP scroll");
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
