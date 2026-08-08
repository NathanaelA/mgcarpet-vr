//! Retail-bug patch switches — the sim half of the `gameplay ·
//! patches` option class (docs/DEVIATIONS.md "Patch options").
//!
//! Each field is one deliberate upstream bugfix with BOTH arms
//! implemented: `true` runs the patched (fixed) behavior, `false`
//! runs retail's shipped bug. The struct is config-like — never part
//! of the state hash or the snapshot stream — and defaults to
//! [`WorldPatches::RETAIL`] at world construction, so every direct
//! `World::new*` consumer (goldens, unit tests, mgc-conform, which
//! never reads app config) evolves under retail law unless the app
//! explicitly opts a patch in. Conformance imports additionally
//! re-force RETAIL as a belt (`World::strict_retail` remains the
//! overriding kill-switch at the gated sites).
//!
//! Reach: `World` methods read `self.patches`; Gen-side ticks get it
//! through [`crate::mc1::mobs::MobCtx::patches`] where a ctx already
//! flows, or as an explicit parameter on the castle/building lanes
//! (the `strict` precedent — a Gen field would drag the wholesale
//! `#[derive(Hash)]` and the snapshot codec along).

/// Per-patch switches; `true` = the patched (bug-fixed) arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorldPatches {
    /// Create Castle pricing (all 3 games). Patched = live-law: the
    /// cost re-derives from the OWN castle every query (homeless →
    /// ctor 1000). Retail = the stale stamp: the manifestation's
    /// cached cost is rewritten at castle init/level-up and NEVER on
    /// castle death (sub_47C60/sub_47DD0; MC2 sub_60780), so a
    /// homeless recast costs the last stamped ladder price — the
    /// player-certified FIRST-CASTLE LOCKOUT.
    pub castle_recast_cost: bool,
    /// Class-12 jars re-snap to their tile's ground every tick.
    /// Retail's reshape walk skips class 12 (:51729): terrain shaped
    /// over/under a jar leaves it buried (HW ships several) or
    /// hovering.
    pub jar_ground_snap: bool,
    /// A settled (f58 == 0) MC1 mana ball tracks the ground both
    /// directions. Retail freezes it wherever it is — mid-hop balls
    /// hang in the air, terrain edits bury grounded ones.
    pub ball_ground_track: bool,
    /// MC1 mana balls run their roll physics map-wide. Retail
    /// re-arms a settled ball's +58 only within the 24-tile awake
    /// radius of the human (:64352-61), so approaching a downhill
    /// ball wakes it and it visibly "runs away". Balls only — the
    /// creature awake gate is untouched.
    pub map_wide_ball_rolling: bool,
    /// A possessed dwelling keeps its footprint extents under the
    /// owner-flag sprite. Retail's sprite stamp (:30808) clobbers
    /// +78..+84 with the tiny flag extent, collapsing villager-emit /
    /// defender spawns onto the roof — a walled-in corpse-flame loop
    /// that destroys the possessed house from the inside.
    pub possessed_footprint: bool,
    /// Total castle destruction routes the residual mana bank
    /// through the ejector's level-0 scatter. Retail's `!level` arm
    /// (:56531-37) frees the castle without ever calling the ejector
    /// — the bank vanishes with the entity (a shipped mana leak).
    pub castle_death_mana: bool,
    /// Total castle destruction demolishes the balloon fleet through
    /// the cull's cargo spill. Retail leaves the balloons flying at
    /// the freed slot's stale coordinates forever.
    pub castle_death_balloons: bool,
    /// MC2 downgrade's 10% capacity haircut computed in i64. Retail's
    /// i32 `10 * x / 100` overflows at the level-7 rung (10 × 300M)
    /// into a NEGATIVE cut — a maxed castle downgrade *raises* its
    /// cap and scatters nothing.
    pub mc2_downgrade_overflow: bool,
    /// MC2 Magic Mine proximity trigger. Retail never writes the
    /// `word_0x36_54` armed gate (magic-mine.md §6) — a shipped mine
    /// floats, expires and sinks without ever detonating on anyone.
    pub mc2_magic_mine: bool,
}

impl WorldPatches {
    /// Every patch off — retail's shipped behavior, bug for bug. The
    /// world-construction default; what conformance, goldens and
    /// `--record`/`--replay` runs use.
    pub const RETAIL: WorldPatches = WorldPatches {
        castle_recast_cost: false,
        jar_ground_snap: false,
        ball_ground_track: false,
        map_wide_ball_rolling: false,
        possessed_footprint: false,
        castle_death_mana: false,
        castle_death_balloons: false,
        mc2_downgrade_overflow: false,
        mc2_magic_mine: false,
    };

    /// The pre-option behavior set: what native play hard-wired
    /// before the patches became options (2026-08-08). Port
    /// recordings taped before the `--record` force-retail policy
    /// replay under THIS set — it is the sim their inputs were
    /// recorded against. `map_wide_ball_rolling` did not exist then.
    pub const LEGACY: WorldPatches = WorldPatches {
        castle_recast_cost: true,
        jar_ground_snap: true,
        ball_ground_track: true,
        map_wide_ball_rolling: false,
        possessed_footprint: true,
        castle_death_mana: true,
        castle_death_balloons: true,
        mc2_downgrade_overflow: true,
        mc2_magic_mine: true,
    };
}
