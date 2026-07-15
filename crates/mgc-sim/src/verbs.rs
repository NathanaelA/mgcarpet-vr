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
//! **State at HEAD**: the MC2 port columns are LANDED (Phases 3-4,
//! the 2026-07-10..12 columns). A few dispatch sites deliberately
//! still note a fallback where the shared MC1 routine IS the serving
//! implementation: the player damage intake (MC2's channels/XP
//! decorators ride the widened combat mail instead) and the MC1-spell
//! acquire paths under an MC2 world — tests/frankenstein.rs pins that
//! ledger. The telemetry stays as the safety net: any arm that isn't
//! natively served degrades gracefully to MC1 and tells the truth in
//! `World::verb_fallbacks`.
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
    /// LIVE — the MC2 two-scan with the type-flag byte policy
    /// (dispatched in `mc1::world`'s awake pass).
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
    /// LIVE — the MC2 roster brain (mc2::roster / mc2::multipart).
    Mc2,
}

/// Projectile targeting / autoaim (survey 3: PARAMETERIZED — same
/// scoring and cone; MC2 extends the subtype key, adds the model-78
/// designated-target pre-acquire and a buildings target source, and
/// moves the homing caps into table data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetingVerb {
    Mc1,
    /// LIVE — the MC2 acquire column (extended subtype key,
    /// designated-target pre-acquire, table-driven homing caps).
    Mc2,
    /// Hidden Worlds: identical to [`TargetingVerb::Mc1`] for every
    /// acquire scan (the creature/wizard/possess cones are all 0x71),
    /// but adds the **model-16 case 0x10** — the Fire Storm fire child
    /// (spell 20 → homing meteor) acquires awake entities within a
    /// widened yaw cone `0x100` (pitch stays `0x71`) and homes. Base
    /// MC1 has no case 16, so its firewall child flies straight
    /// (fire-rain). remc1hw acquire switch :60322; docs/SURVEY-MC1HW.md
    /// §3a. Consumed in [`crate::mc1::combat`]'s `proj_firewall_tick`;
    /// every other targeting site treats it exactly as `Mc1`. Ordered
    /// LAST so `Mc1`/`Mc2` keep their discriminants (the VerbSet feeds
    /// the state hash — a reorder would move every MC2 golden).
    Mc1Hw,
}

/// Damage application / player intake (survey 2: PARAMETERIZED —
/// byte-identical mailbox protocol; MC2 adds intake channels and the
/// spell-XP DECORATORS on the same combat events, plus per-game
/// sounds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageVerb {
    Mc1,
    /// LIVE — the MC2 intake channels + spell-XP decorators ride the
    /// widened (owner, spell, amount) combat mail; the PLAYER-intake
    /// dispatch itself still serves the shared MC1 routine and notes
    /// the fallback (tests/frankenstein.rs pins it).
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
    /// LIVE — the MC2 stage/objective machine (mc2::stagevars +
    /// the stage engine), including the spawn-side stage binding
    /// hooked at the spawn seam per the SCOPE NOTE above.
    Mc2,
}

/// The corpse pipeline (survey 2: PARAMETERIZED — MC1 scatters mana
/// jars/balls; MC2 scatters spell tokens and splits/merges mana
/// spheres).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorpseVerb {
    Mc1,
    /// LIVE — MC2 spell-token scatter + mana-sphere split/merge, in
    /// the mc2 death handlers (not routed through `corpse_drop`; an
    /// MC2 world reaching the MC1 drop notes the fallback).
    Mc2,
}

/// The player movement-commit gate — THE one genuinely REWRITTEN
/// verb (survey 4): MC1's type-8 wall rule (impassable at any
/// altitude, cardinal slide) vs MC2's water/blocked-flag/cave-steer,
/// which also zeroes target speed on block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitGateVerb {
    Mc1,
    /// LIVE — the MC2 water/blocked-flag/cave-steer gate
    /// (`player_mc2_gate`), zeroing target speed on block.
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
    /// LIVE — the MC2 mover (`crate::flight::mc2_move`, Phase 4.4:
    /// tuning row 66/104, clearance 256, cave-only speed-zero).
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

    /// The Hidden Worlds column — the MC1 column with the one live HW
    /// engine divergence flipped: [`TargetingVerb::Mc1Hw`] (the Fire
    /// Storm homing meteor, docs/SURVEY-MC1HW.md §3a). Everything else
    /// is MC1; the spell-20 stat rebalance is data (spells table), the
    /// napalm-geometry fork is a separate handler branch.
    pub const MC1HW: VerbSet = VerbSet {
        awake: AwakeVerb::Mc1,
        movement: MovementVerb::Mc1,
        targeting: TargetingVerb::Mc1Hw,
        damage: DamageVerb::Mc1,
        objective: ObjectiveVerb::Mc1,
        corpse: CorpseVerb::Mc1,
        commit_gate: CommitGateVerb::Mc1,
        flight: FlightVerb::Mc1,
    };

    /// The pristine MC2 column — every arm LIVE at HEAD (the
    /// Phase 3-4 port columns; `verb_fallbacks` reports none).
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
