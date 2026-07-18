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

// ===================================================================
// The campaign LAW — level order, exit routing, progression state.
// (Everything above is the plausible-spellbook playtest instrument;
// everything below is the real campaign driver's rulebook. Traces:
// docs/traces/mc1-campaign-save-menu.md, mc2-campaign-save-menu.md.)
// ===================================================================

use crate::saves::SecretPortal;

/// Which campaign is running — the `--campaign <mc1|mc1hw|mc2>` pick.
/// Doubles as the baked-tree tag and the saves-directory name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignId {
    Mc1,
    Mc1Hw,
    Mc2,
}

impl CampaignId {
    pub fn tag(self) -> &'static str {
        match self {
            CampaignId::Mc1 => "mc1",
            CampaignId::Mc1Hw => "mc1hw",
            CampaignId::Mc2 => "mc2",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mc1" => Some(CampaignId::Mc1),
            "mc1hw" => Some(CampaignId::Mc1Hw),
            "mc2" => Some(CampaignId::Mc2),
            _ => None,
        }
    }
}

/// What follows a completed level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextStep {
    /// Load this level next (MC1's linear march; MC2's secret-exit
    /// direct jump).
    Level(u32),
    /// MC2: return to the world map (the between-levels hub).
    MapScreen,
    /// The campaign is complete — the outro slot.
    Outro,
}

/// MC1/HW: the strictly linear advance. Retail bumps `var_u16_17`
/// on the win bit (remc1 :41601) and then, at the NEXT level's start,
/// bumps again past the skip table (`sub_34070` :41456-73, exact
/// match, MC1 only — HW plays every index). Campaign ends at 50
/// (MC1) / 25 (HW) (`:59939-41`, `:60147`).
pub fn mc1_next_level(completed: u32, hw: bool) -> NextStep {
    let end = if hw { 25 } else { 50 };
    let mut next = completed + 1;
    if !hw && MC1_BLACKLIST.contains(&next) {
        next += 1; // retail's single ++: the table has no adjacent entries
    }
    if next >= end {
        NextStep::Outro
    } else {
        NextStep::Level(next)
    }
}

/// Normalize a saved/starting MC1 level into the first PLAYABLE
/// campaign level at or after it (retail's `sub_34070` runs at level
/// START, so a save sitting on a skipped index bumps forward exactly
/// like retail). None = the campaign is already complete.
pub fn mc1_start_level(saved: u32, hw: bool) -> Option<u32> {
    let end = if hw { 25 } else { 50 };
    let mut level = saved;
    if !hw && MC1_BLACKLIST.contains(&level) {
        level += 1;
    }
    (level < end).then_some(level)
}

/// One MC2 world-map main portal: the map-scroll anchor, the portal
/// sprite position, and the hit-box size — verbatim
/// `mapScreenPortals_E17CC` init (Type_MapScreenPortals_E17CC.cpp:3;
/// portal index == level number). All 25 start hidden (activated 2),
/// sprite 0x21 = 33, hit-box 0x28×0x28.
#[derive(Clone, Copy, Debug)]
pub struct Mc2MainPortal {
    pub viewport: (i16, i16),
    pub pos: (i16, i16),
}

/// The 25 main-campaign portals, index = level number; index 24 is
/// the finale (its completion triggers the ending, EF:31505-06).
pub const MC2_MAIN_PORTALS: [Mc2MainPortal; 25] = {
    const fn p(vx: i16, vy: i16, x: i16, y: i16) -> Mc2MainPortal {
        Mc2MainPortal {
            viewport: (vx, vy),
            pos: (x, y),
        }
    }
    [
        p(116, 478, 420, 820),
        p(368, 478, 666, 805),
        p(576, 478, 881, 734),
        p(260, 402, 549, 626),
        p(260, 402, 450, 652),
        p(304, 402, 610, 666),
        p(304, 402, 763, 652),
        p(304, 402, 732, 558),
        p(304, 402, 644, 554),
        p(304, 402, 536, 540),
        p(512, 306, 822, 450),
        p(638, 190, 1009, 412),
        p(638, 92, 1058, 268),
        p(478, 92, 901, 304),
        p(478, 92, 817, 202),
        p(478, 92, 684, 262),
        p(122, 96, 530, 316),
        p(122, 96, 427, 206),
        p(122, 96, 322, 254),
        p(306, 196, 627, 416),
        p(1, 68, 180, 278),
        p(296, 68, 609, 218),
        p(480, 0, 838, 96),
        p(308, 0, 679, 126),
        p(308, 0, 605, 120),
    ]
};

/// The portal hit-box size (`word_8/10` = 0x28 both axes).
pub const MC2_PORTAL_HIT: i16 = 0x28;

/// The five MC2 secret levels `(parent main level, secret level,
/// map pos)` — verbatim `secretMapScreenPortals_E2970` init
/// (Type_SecretMapScreenPortals_E2970.cpp:3).
pub const MC2_SECRETS: [(u16, u16, (u16, u16)); 5] = [
    (4, 30, (287, 656)),
    (7, 31, (879, 614)),
    (11, 32, (854, 400)),
    (17, 33, (395, 114)),
    (19, 34, (365, 504)),
];

/// The pristine (new-game) secret-portal table in its on-disk shape:
/// all hidden (activated 3, sprite 270 as initialized — the reset arm
/// re-stamps sprite 70), terminator entry zeroed. This is both the
/// new-game state and the default `.GAM` block.
pub fn mc2_secret_portals_pristine() -> [SecretPortal; 6] {
    let mut out = [SecretPortal {
        time: 0,
        parent: 0,
        level: 0,
        pos: (0, 0),
        activated: 0,
        sprite: 0,
        byte16: 0,
    }; 6];
    for (i, &(parent, level, pos)) in MC2_SECRETS.iter().enumerate() {
        out[i] = SecretPortal {
            time: 0,
            parent,
            level,
            pos,
            activated: 3,
            sprite: 270,
            byte16: 0,
        };
    }
    out
}

/// The secret level attached to a main level, if any
/// (`GetSecretAndActivedPortal_824B0` EF:46992 matches `index_4`).
pub fn mc2_secret_for(main_level: u32) -> Option<u32> {
    MC2_SECRETS
        .iter()
        .find(|&&(parent, _, _)| parent as u32 == main_level)
        .map(|&(_, level, _)| level as u32)
}

/// Is this MC2 level number a secret level (EF:31407)?
pub fn mc2_is_secret(level: u32) -> bool {
    (25..50).contains(&level) // retail: > 24 && < 50; only 30-34 exist
}

/// MC2 exit routing: what follows a completed level, given WHICH
/// ending marker ran (the endseq target model: 3 = the (14,3)
/// checkpoint X / action 12, 4 = the (14,4) demon mouth / action 11)
/// and whether this level's secret portal is revealed-uncompleted.
///
/// Retail law (EF:60534-44 sets `byte[2]`, EF:31510-48 consumes):
/// the demon-mouth exit ALWAYS routes into the attached secret level;
/// the checkpoint exit routes there only when the secret portal was
/// already revealed but not completed (`setting_38545 & 0x10`) —
/// otherwise back to the map for the linear advance. Completing the
/// finale (24) or a secret level returns to the map (the finale's
/// ending runs first; a secret level has no onward jump of its own).
pub fn mc2_next_step(level: u32, exit_model: u8, secret_pending: bool) -> NextStep {
    if level < 24 && !mc2_is_secret(level) {
        let into_secret = exit_model == 4 || secret_pending;
        if into_secret && let Some(s) = mc2_secret_for(level) {
            return NextStep::Level(s);
        }
    }
    if level == 24 {
        return NextStep::Outro;
    }
    NextStep::MapScreen
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
    fn mc1_linear_order_applies_skips_and_ends() {
        // 7 → skip 8 → 9 (the retail sub_34070 exact-match bump).
        assert_eq!(mc1_next_level(7, false), NextStep::Level(9));
        assert_eq!(mc1_next_level(16, false), NextStep::Level(18));
        assert_eq!(mc1_next_level(38, false), NextStep::Level(40));
        // Plain advance elsewhere.
        assert_eq!(mc1_next_level(0, false), NextStep::Level(1));
        // Ends at 50; 49 is the last played level.
        assert_eq!(mc1_next_level(49, false), NextStep::Outro);
        // HW: no skips, ends at 25.
        assert_eq!(mc1_next_level(7, true), NextStep::Level(8));
        assert_eq!(mc1_next_level(24, true), NextStep::Outro);
    }

    #[test]
    fn mc2_routing_demon_mouth_jumps_into_secret() {
        // Level 4's demon mouth (model 4) → secret 30, always.
        assert_eq!(mc2_next_step(4, 4, false), NextStep::Level(30));
        // The checkpoint X (model 3) → map, unless the secret portal
        // was already revealed-uncompleted (the traced 38545&0x10 arm).
        assert_eq!(mc2_next_step(4, 3, false), NextStep::MapScreen);
        assert_eq!(mc2_next_step(4, 3, true), NextStep::Level(30));
        // A level with no attached secret never jumps.
        assert_eq!(mc2_next_step(5, 4, false), NextStep::MapScreen);
        // Secret levels and the finale return to map / outro.
        assert_eq!(mc2_next_step(30, 3, false), NextStep::MapScreen);
        assert_eq!(mc2_next_step(24, 3, false), NextStep::Outro);
    }

    #[test]
    fn mc2_secret_table_matches_retail_init() {
        assert_eq!(mc2_secret_for(4), Some(30));
        assert_eq!(mc2_secret_for(19), Some(34));
        assert_eq!(mc2_secret_for(0), None);
        let p = mc2_secret_portals_pristine();
        assert_eq!(p[2].level, 32);
        assert_eq!(p[2].parent, 11);
        assert_eq!(p[2].activated, 3, "pristine = hidden");
        assert_eq!(p[5].level, 0, "terminator entry zeroed");
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
