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
    /// **MC1/HW's FIRST-CASTLE LOCKOUT — an unpatched retail BUG.**
    /// Patched = live-law: the cost re-derives from the OWN castle
    /// every query (homeless → ctor 1000). Retail = the stale stamp:
    /// the manifestation's cached cost is rewritten at castle
    /// init/level-up and NEVER on castle death (sub_47C60/sub_47DD0),
    /// so after ANY castle loss the rebuild is priced at
    /// `CASTLE_CAP[0]` = **5,000** against a **1,000** starting purse
    /// — unaffordable until collection pushes the census ceiling past
    /// it. Player-certified on retail; the lockout may be a deliberate
    /// period challenge, so the FIX is the opt-in (player-ruled
    /// 2026-08-08, DEFAULT RETAIL).
    ///
    /// ⚠⚠ **THIS IS AN MC1/HW CONCERN.** MC2 does NOT have the
    /// lockout: its destroy path re-stamps the token at the level-0
    /// rung (**1,000**) before the record frees, so an MC2 castle
    /// destroyed by an enemy is immediately rebuildable. MC2 charges
    /// for teardown through the DESIGNED +3,000 surcharge instead,
    /// which only a VOLUNTARY demolish latches — that is deliberate
    /// behaviour, it does NOT ride this switch, and it is faithful
    /// under BOTH arms (mc2/cast.rs). The one MC2 thing left here is a
    /// narrow corner: the castle-less RELEASE re-sync in mc2/cast.rs,
    /// a release edge with no castle and no intervening ladder stamp.
    /// Do not extend this toggle to cover MC2 design.
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
        mc2_downgrade_overflow: false,
        castle_latch_bug: false,
        no_spell_loss: false,
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
        mc2_downgrade_overflow: true,
        castle_latch_bug: true,
        no_spell_loss: false,
    };
}
