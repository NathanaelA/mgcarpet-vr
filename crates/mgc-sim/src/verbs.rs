//! The tier-5 verb seam — the superset sim's per-game dispatch
//! surface (ROADMAP "MULTI-GAME ARCHITECTURE", Phase 2; verb
//! inventory + verdicts: docs/SURVEY-MC2.md "Consolidated tier-5
//! verb inventory").
//!
//! One enum per swappable engine verb, bundled in a [`VerbSet`] the
//! sim takes at construction (like [`crate::chassis::ChassisParams`])
//! and never rebranches on outside this seam. The dispatch rule is
//! the tier taxonomy's: variation lives HERE (tier-3 wiring), never
//! as an `if mc2` inside a handler — a differing routine is two
//! arms. [`crate::flight`] is the precedent: per-game movers behind
//! an enum picked at the boundary, replay-recordable.
//!
//! **Phase-2 state**: every MC2 arm is declared but PENDING — its
//! dispatch falls back to the MC1 implementation and notes the
//! fallback (graceful degradation at the seam: never crash, tell the
//! truth in telemetry). Phase 3 lands the real arms one verb at a
//! time; the dispatch match in this file is exactly where that code
//! change goes.
//!
//! Two inventory rows deliberately carry NO enum:
//! - **Tick orchestration** — the 4-phase skeleton is IDENTICAL
//!   cross-game (survey 4); its variance is already chassis data
//!   (bucket count/predicates, win debounce) plus a pre-pass hook
//!   list that joins when MC2's extra pre-passes are traced.
//! - **LOS / height sampling** — the algorithm is IDENTICAL; MC2's
//!   delta is DATA, not code: cave levels carry a second (ceiling)
//!   heightmap, which lands as an optional plane on
//!   [`crate::mc1::features::Planes`] when the MC2 cave slice is
//!   ported, widening the sampler to a floor/ceiling pair.

/// Creature awake / sight-aggro (survey 3: PARAMETERIZED — the same
/// shared two-scan; MC2 keys the scan off a type-flag byte, bits
/// 4/8, and widens the retaliation class policy).
///
/// SHARED-INPUT RULE: the type-flag byte is the MC2 behavior row's
/// trailing flags byte (bit 1 die-on-water, bit 4 pack-disable,
/// bit 8 flee/alt-chase — remc2 :8855/:9022/:9003), which the
/// [`MovementVerb`] arm reads too. Both MC2 arms must consume the
/// SAME extracted MC2 row table — landing one against MC1 rows
/// produces a silently inconsistent creature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AwakeVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`AwakeVerb::Mc1`].
    Mc2,
}

/// Creature brain / movement family — the class-5 state machine
/// behind the per-model dispatch (survey 4: PARAMETERIZED skeleton;
/// MC2 widens the behavior row by a flags byte, adds the FLEE
/// primitive, and passes the attack thunk as a parameter — Bullfrog
/// themselves moved toward this design).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MovementVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`MovementVerb::Mc1`].
    Mc2,
}

/// Projectile targeting / autoaim (survey 3: PARAMETERIZED — same
/// scoring and cone; MC2 extends the subtype key, adds the model-78
/// designated-target pre-acquire and a buildings target source, and
/// moves the homing caps into table data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetingVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`TargetingVerb::Mc1`].
    Mc2,
}

/// Damage application / player intake (survey 2: PARAMETERIZED —
/// byte-identical mailbox protocol; MC2 adds intake channels and the
/// spell-XP DECORATORS on the same combat events, plus per-game
/// sounds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`DamageVerb::Mc1`].
    Mc2,
}

/// The win/objective engine (survey 2: the mana census is shared in
/// shape; the WIN CHECK is rewritten — MC1's banked-share streak vs
/// MC2's stage/objective machine, which latches immediately).
///
/// SCOPE NOTE from the Phase-3 spec review: MC2's machine is not
/// only a win check — its SPAWN-SIDE half (`sub_58DA0` binds every
/// spawned entity into its stage slot so kill-objectives can find
/// their target; a stage-var pre-pass reacts to fired dispositions,
/// remc2 :32967/:40650) runs inside the spawn path and the tick
/// pre-pass. When the Mc2 arm lands, that binding needs an explicit
/// hook at the spawn seam — NOT an `if mc2` inside the spawn
/// dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`ObjectiveVerb::Mc1`].
    Mc2,
}

/// The corpse pipeline (survey 2: PARAMETERIZED — MC1 scatters mana
/// jars/balls; MC2 scatters spell tokens and splits/merges mana
/// spheres).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorpseVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`CorpseVerb::Mc1`].
    Mc2,
}

/// The player movement-commit gate — THE one genuinely REWRITTEN
/// verb (survey 4): MC1's type-8 wall rule (impassable at any
/// altitude, cardinal slide) vs MC2's water/blocked-flag/cave-steer,
/// which also zeroes target speed on block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitGateVerb {
    Mc1,
    /// PENDING (Phase 3): falls back to [`CommitGateVerb::Mc1`].
    Mc2,
}

/// The faithful player-flight model this game means by "faithful".
/// Orthogonal to [`crate::ThrustModel`], which picks faithful vs the
/// enhanced deviation; this picks WHICH faithful.
///
/// CORRECTED after the Phase-3 spec review (Opus, 2026-07-09; the
/// survey's "parameterized" verdict was optimistic): the input
/// filter/rates/±16/80 speeds DO match, but MC2's climb authority is
/// a DIFFERENT formula — a behavior-row-driven linear ramp
/// (`((z − ground − row.word_0xa) << 10) / row.word_0xa`, clamp
/// ±256; remc2 EventsFunctions.cpp:59645) where MC1 uses piecewise
/// constants — and the MC2 mover reads player-extension state MC1
/// has no slot for (slow-effect scaling, full-stop mobilize counter,
/// strafe/boost channels; :59610-59699). The MC2 arm is a real port
/// with its own state struct, not a re-parameterization of
/// [`crate::flight::mc1_move`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlightVerb {
    Mc1,
    /// PENDING (Phase 4 — deferred out of the Phase-3 slice, see
    /// ROADMAP): falls back to [`FlightVerb::Mc1`].
    Mc2,
}

/// Telemetry key for a verb whose requested arm is pending and fell
/// back to MC1 (one note per verb per world; surfaced by
/// `World::verb_fallbacks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbKind {
    Awake = 0,
    Movement = 1,
    Targeting = 2,
    Damage = 3,
    Objective = 4,
    Corpse = 5,
    CommitGate = 6,
    Flight = 7,
}

impl VerbKind {
    pub const fn name(self) -> &'static str {
        match self {
            VerbKind::Awake => "awake",
            VerbKind::Movement => "movement",
            VerbKind::Targeting => "targeting",
            VerbKind::Damage => "damage",
            VerbKind::Objective => "objective",
            VerbKind::Corpse => "corpse",
            VerbKind::CommitGate => "commit-gate",
            VerbKind::Flight => "flight",
        }
    }

    pub(crate) const ALL: [VerbKind; 8] = [
        VerbKind::Awake,
        VerbKind::Movement,
        VerbKind::Targeting,
        VerbKind::Damage,
        VerbKind::Objective,
        VerbKind::Corpse,
        VerbKind::CommitGate,
        VerbKind::Flight,
    ];
}

/// The per-game verb selection, fixed at world construction. Replays
/// must record it (with the chassis set) once replays exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerbSet {
    pub awake: AwakeVerb,
    pub movement: MovementVerb,
    pub targeting: TargetingVerb,
    pub damage: DamageVerb,
    pub objective: ObjectiveVerb,
    pub corpse: CorpseVerb,
    pub commit_gate: CommitGateVerb,
    pub flight: FlightVerb,
}

impl VerbSet {
    /// The pristine MC1 column.
    pub const MC1: VerbSet = VerbSet {
        awake: AwakeVerb::Mc1,
        movement: MovementVerb::Mc1,
        targeting: TargetingVerb::Mc1,
        damage: DamageVerb::Mc1,
        objective: ObjectiveVerb::Mc1,
        corpse: CorpseVerb::Mc1,
        commit_gate: CommitGateVerb::Mc1,
        flight: FlightVerb::Mc1,
    };

    /// The pristine MC2 column — every arm pending in Phase 2 (falls
    /// back to MC1 with telemetry); Phase 3 lands them one by one.
    pub const MC2: VerbSet = VerbSet {
        awake: AwakeVerb::Mc2,
        movement: MovementVerb::Mc2,
        targeting: TargetingVerb::Mc2,
        damage: DamageVerb::Mc2,
        objective: ObjectiveVerb::Mc2,
        corpse: CorpseVerb::Mc2,
        commit_gate: CommitGateVerb::Mc2,
        flight: FlightVerb::Mc2,
    };
}

impl Default for VerbSet {
    fn default() -> Self {
        VerbSet::MC1
    }
}
