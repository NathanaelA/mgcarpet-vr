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
    /// Class-12 jars re-snap to their tile's ground every tick.
    /// Retail's reshape walk skips class 12 (:51729): terrain shaped
    /// over/under a jar leaves it buried (HW ships several) or
    /// hovering.
    pub jar_ground_snap: bool,
    /// A settled (f58 == 0) MC1 mana ball tracks the ground both
    /// directions. Retail freezes it wherever it is — mid-hop balls
    /// hang in the air, terrain edits bury grounded ones.
    pub ball_ground_track: bool,
    /// Mana balls run their roll physics map-wide, BOTH games. Retail
    /// re-arms a settled ball's +58 only within the 24-tile awake
    /// radius of the human (MC1 :64352-61; MC2 `sub_68C70`'s sphere
    /// leg, EF:55526), so approaching a downhill ball wakes it and it
    /// visibly "runs away". Balls only — the creature awake gate is
    /// untouched. (MC1-only until 2026-09-06; the MC2 arm was
    /// player-requested alongside `ball_owner_recolor`, which stays a
    /// separate option: a settled ball that never wakes still wants
    /// its owner's colour.)
    pub map_wide_ball_rolling: bool,
    /// A possessed dwelling keeps its footprint extents under the
    /// owner-flag sprite. Retail's sprite stamp (:30808) clobbers
    /// +78..+84 with the tiny flag extent, collapsing villager-emit /
    /// defender spawns onto the roof — a walled-in corpse-flame loop
    /// that destroys the possessed house from the inside.
    pub possessed_footprint: bool,
    /// MC2 downgrade's 10% capacity haircut computed in i64. Retail's
    /// i32 `10 * x / 100` overflows at the level-7 rung (10 × 300M)
    /// into a NEGATIVE cut — a maxed castle downgrade *raises* its
    /// cap and scatters nothing.
    pub mc2_downgrade_overflow: bool,
    /// MC1 Create Castle placement validation. Retail (the "latch
    /// bug", certified on mc1l32-castle-bug.mgcr): the castle ball
    /// spawns at the HAND muzzle (sub_55EF0 — ±256 units at yaw∓512,
    /// steerable by aim), the launch-tick scan samples THAT tile, and
    /// the landing condition short-circuits (`ground > z || life < 0
    /// || !scan`, :63588-90) — a terrain touchdown builds the castle
    /// with no placement check at all; the scan only re-runs on
    /// airborne ticks, where a failure stops the ball early
    /// (flip 180° + one step back) and still builds. Combined with
    /// sub_12F70's NW-only 8×8 window this lets a wall-corner cast
    /// raise a castle inside a no-castle maze and carve its protected
    /// walls. Patched: the ball spawns at the carpet (the scan
    /// samples where you are) and the landing always re-scans
    /// (failure displaces the site one step back).
    pub castle_latch_bug: bool,
    /// **THE HUMAN'S SPELLBOOK IS PERMANENT** (player-ruled
    /// 2026-09-07, an unfaithful patch, DEFAULT ON, both games).
    /// Retail can lose a spell the wizard already holds, and the
    /// design never earns it back: MC1 has no legitimate way to lose
    /// a spell, MC2 exactly one — the undead wraith's steal
    /// (`mc2_spell_steal`), which drops the jar for re-collection.
    /// Everything else is the abandoned death-scatter mechanism and
    /// its fallout, witnessed on mc1l49: ⑴ the respawn re-mint is
    /// one-shot and a starved mint zeroes the bank entry (:54917 /
    /// EF:59402); ⑵ WITHOUT a death, a stale entity handle kills the
    /// owned token — mc1l49 t=17809: the volcano's plume register
    /// still named slot 235, which the last respawn had re-minted as
    /// the wall-of-fire token, so the next eruption soft-killed it
    /// (`combat.rs` volcano arm, retail law), the exhausted pool
    /// recycled the slot into one of the wizard's own fires a tick
    /// later, the blind list read registered a second fireball, and
    /// with the 24-entry list FULL of stale slots every wall-of-fire
    /// jar the rivals dropped for the rest of the level was refused
    /// (:64841) — "a map littered with jars I could not pick up".
    ///
    /// Patched: the death scatter throws COSMETIC decaying jars and
    /// leaves the tokens in the book (so nothing is re-minted on
    /// respawn either); a soft-kill of a live human token is refused
    /// at `World::free_slot` (the tick-top reap and the MC2 in-loop
    /// free) and the token is scrubbed off the sacrifice stack; a
    /// stale MC1 list entry that no longer names a class-12 record
    /// is cleared by the per-tick rebuild so a re-dropped jar can be
    /// picked up. The wraith steal is untouched — it is the one
    /// explicit, designed inventory writer.
    pub no_spell_loss: bool,
    /// MC1 body segments (the dragon's 16-piece tail, the sea
    /// serpent's, the two-piece m6) keep their rigid follow while
    /// ASLEEP. Retail's `sub_19550` (:21107) only walks an awake
    /// segment to `+56` behind its leader; an asleep one collapses
    /// onto the leader every 4th tick — a period CPU save behind the
    /// 24-tile awake radius that reads as "the tail lags in strange
    /// ways" once the port's fog shows distant dragons. This is the
    /// only reason anyone raised `awake_range`, which also wakes the
    /// whole ecology (player-ruled 2026-09-07, DEFAULT ON). The
    /// damage intake stays awake-gated.
    pub mc1_fix_dragon_tail: bool,
    /// **MC2's PHANTOM CASTLE AT THE MAP ORIGIN** (player-reported
    /// 2026-09-10, DEFAULT ON). Retail's building-completion tail
    /// `sub_377A0` castle-re-paints EVERY class-3 record overlapping
    /// the finished plot, model unchecked, minting a (10,42) painter
    /// at that record's spare axis `@0x9A` with its `@0x10` word as
    /// the BUILD00 row. On a castle those are the site and the level;
    /// on a dead rival wizard they are a never-scouted (0,0,0) and the
    /// respawn countdown — a corpse lying on a village re-raises
    /// castle rows at the origin, at sea level, every time a hut
    /// finishes under it (mc2l22 t=20868; on demand on mc2l1: kill
    /// Nyphur over the mana-magnet basins, destroy a dwelling). Both
    /// games' retail shows some of it (the outline for countdown bytes
    /// 7..76). Patched: only castles (3,2) are re-painted.
    pub mc2_phantom_castle: bool,
}

impl WorldPatches {
    /// Every patch off — retail's shipped behavior, bug for bug. The
    /// world-construction default; what conformance, goldens and
    /// `--record`/`--replay` runs use.
    pub const RETAIL: WorldPatches = WorldPatches {
        jar_ground_snap: false,
        ball_ground_track: false,
        map_wide_ball_rolling: false,
        possessed_footprint: false,
        mc2_downgrade_overflow: false,
        castle_latch_bug: false,
        no_spell_loss: false,
        mc1_fix_dragon_tail: false,
        mc2_phantom_castle: false,
    };

    /// The pre-option behavior set: what native play hard-wired
    /// before the patches became options (2026-08-08). Port
    /// recordings taped before the `--record` force-retail policy
    /// replay under THIS set — it is the sim their inputs were
    /// recorded against. `map_wide_ball_rolling` did not exist then.
    /// (`castle_recast_cost`, live-law castle pricing, was part of
    /// this set until its retirement on 2026-09-07 — player-ruled:
    /// the lockout is retail's and the relief made the early game
    /// too easy; such recordings replay under the lockout now.)
    pub const LEGACY: WorldPatches = WorldPatches {
        jar_ground_snap: true,
        ball_ground_track: true,
        map_wide_ball_rolling: false,
        possessed_footprint: true,
        mc2_downgrade_overflow: true,
        castle_latch_bug: true,
        no_spell_loss: false,
        mc1_fix_dragon_tail: false,
        mc2_phantom_castle: false,
    };
}
