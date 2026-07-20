//! Chassis parameters — the per-game constant set of the SHARED sim
//! chassis (the full diff: docs/archive/SURVEY-MC2.md "Proposed
//! ChassisParams").
//!
//! MC1 and MC2 run the SAME chassis — pool/allocator (two-stack,
//! 999→1 build, LIFO, fail-open exhaustion), LCG constants 9377/9439
//! with slot+global per-entity seeding, tile chains, the 6-channel
//! damage-mailbox protocol, the 4-phase tick skeleton — and these
//! values are the only chassis-level differences. Each game defines
//! its PRISTINE set; the sim takes one at construction and never
//! branches on "which game" elsewhere.
//!
//! Deviating from a pristine set is the LIMIT-REMOVING option class:
//! e.g. a bumped `pool_slots` un-drops the spawns retail silently
//! discarded — bit-identical to retail up to the first exhaustion
//! event, divergent after (and the win goal moves, since dropped
//! spawns carry mana). Replays must record the set.

/// Per-entity LCG state width. Same constants either way; MC2 keeps
/// the seed as u16 (entity offset 20 vs MC1's u32 at offset 4), so
/// its stream runs mod 2^16 (9377 ≡ 1 mod 4 keeps it full-period).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RandWidth {
    U32,
    U16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChassisParams {
    /// Level THING-table capacity, records addressed 1-based (MC1
    /// scans 1..=1999; MC2 shrank the table to 0x4B0 = 1200). Ours
    /// pads MC1 to 2096 — headroom for community-authored slots.
    pub level_table_slots: usize,
    /// Runtime event-pool size, slot 0 never allocated. 1000 in BOTH
    /// retail engines; >1000 = the limit-removing bump (see module
    /// doc). The fail-open exhaustion behavior itself is chassis,
    /// not a parameter.
    pub pool_slots: usize,
    /// Per-entity LCG width (see [`RandWidth`]).
    pub ent_rand_width: RandWidth,
    /// Per-model creature bucket heads in the tick pre-pass (MC1 20,
    /// MC2 29); model indices clamp to the last bucket.
    pub bucket_models: usize,
    /// Creature states EXCLUDED from the buckets (multipart body
    /// segments etc.): MC1 {120}, MC2 {0xB4, 0xE8, 0xEA}.
    pub bucket_excluded_states: &'static [u8],
    /// Consecutive over-threshold ticks the banked-% win check needs
    /// (MC1 16; MC2's objective engine latches immediately).
    pub win_streak_ticks: u16,
    /// The creature awake gate, squared engine units: the awake
    /// pre-pass re-arms a sleeping creature's f58 when the player is
    /// closer than this (dist² < gate). 0x240_0000 = 24 tiles in
    /// BOTH retail engines (remc1 sub_54F80; remc2 EF:55526 — same
    /// literal). A raised gate is the limit-removing class like
    /// `pool_slots` (sleep was a period CPU optimization: asleep
    /// creatures still MOVE but don't scan, take mail damage, or
    /// re-derive segment spacing); `i32::MAX` = always awake.
    pub awake_gate_sq: i32,
}

impl ChassisParams {
    /// The pristine MC1 set (remc1, byte-faithful defaults).
    pub const MC1: ChassisParams = ChassisParams {
        level_table_slots: 2096,
        pool_slots: 1000,
        ent_rand_width: RandWidth::U32,
        bucket_models: 20,
        bucket_excluded_states: &[120],
        win_streak_ticks: 16,
        awake_gate_sq: 0x240_0000,
    };

    /// The pristine MC2 set (remc2 survey values).
    pub const MC2: ChassisParams = ChassisParams {
        level_table_slots: 1200,
        pool_slots: 1000,
        ent_rand_width: RandWidth::U16,
        bucket_models: 29,
        bucket_excluded_states: &[0xB4, 0xE8, 0xEA],
        win_streak_ticks: 1,
        awake_gate_sq: 0x240_0000,
    };
}

impl Default for ChassisParams {
    fn default() -> Self {
        ChassisParams::MC1
    }
}

/// Manual Hash (not derived): the chassis is hashed inside `Gen`, so a
/// NEW field must be hash-transparent at its pristine value or every
/// golden moves for a layout-only reason (a future field addition is a
/// compile error in the destructure below — extend deliberately). A
/// deviating gate (`--awake-range`) hashes with a tag byte: deviated
/// runs are hash-visible, as replays require.
impl std::hash::Hash for ChassisParams {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        let ChassisParams {
            level_table_slots,
            pool_slots,
            ent_rand_width,
            bucket_models,
            bucket_excluded_states,
            win_streak_ticks,
            awake_gate_sq,
        } = self;
        // The pre-field stream, byte-identical (declaration order).
        (
            level_table_slots,
            pool_slots,
            ent_rand_width,
            bucket_models,
            bucket_excluded_states,
            win_streak_ticks,
        )
            .hash(h);
        if *awake_gate_sq != 0x240_0000 {
            h.write_u8(0xC5);
            awake_gate_sq.hash(h);
        }
    }
}

// ------------------------------------------------------------ snapshot
//
// Only `RandWidth` travels: `ChassisParams` as a whole is fixed at
// construction and identity-CHECKED rather than restored, because its
// `bucket_excluded_states` is a `&'static [u8]` with nowhere to land.

use crate::snapshot::snap_enum;

snap_enum!(RandWidth, "RandWidth", 0 => RandWidth::U32, 1 => RandWidth::U16);
