//! The MC1 (Magic Carpet 1 / Hidden Worlds) simulation — every
//! module here is a verbatim port of remc1 machinery (ROADMAP
//! "MULTI-GAME ARCHITECTURE": tier-2 tables, tier-3/4 dispatch +
//! handlers, and the tier-5 engine verbs, pending their Phase-2
//! seam). The game-agnostic pieces live OUTSIDE this namespace:
//! [`crate::chassis`] (the shared-engine parameter sets) and
//! [`crate::flight`] (the flight-model seam, MC1 + enhanced tiers).
//!
//! Hidden Worlds is NOT a separate namespace: retail ships it as a
//! sibling binary of the same engine, and it consumes this module
//! with its own asset bundles (a small behavior-delta set may join
//! later — none identified yet).

pub mod behavior;
pub(crate) mod combat;

/// The MC1 THING registry column ([`crate::ids::GameId::known_thing`]):
/// the `(class, model)` set this game's spawn dispatch understands —
/// including its AUTHENTIC no-spawns (class-10 null/stub creators,
/// class-3 start markers), which are known non-entities, not misfits.
/// Derived from the spawn guards in `mobs.rs` (`spawn_scenery`
/// model ≤ 5, `spawn_class3` model ≤ 11, `spawn_creature` model ≤ 16),
/// `features.rs` (`spawn_creator` model ≤ 61) and `world.rs`
/// (`spawn_from_thing`'s class dispatch, `spawn_trigger` states).
pub(crate) fn known_thing(class: u16, model: u16) -> bool {
    match class {
        2 => model <= 5,
        3 => model <= 11,
        5 => model <= 16,
        // Spawner logic / authored spell effects / jars: any model —
        // classes 7/9 park inert, class-12 models are jar variants.
        7 | 9 | 12 => true,
        10 => model <= 61,
        // Trigger volumes: model = the trigger state machine
        // (0..=30 dispatched; 31 = the advertised X-marker family).
        11 => model <= 31,
        _ => false,
    }
}
pub mod corners;
pub mod entities;
pub mod features;
pub(crate) mod mobs;
pub mod rivals;
pub mod spells;
pub mod sprite_stats;
pub(crate) mod tables;
pub mod world;
