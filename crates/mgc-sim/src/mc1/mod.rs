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
pub mod corners;
pub mod entities;
pub mod features;
pub(crate) mod mobs;
pub mod rivals;
pub mod spells;
pub mod sprite_stats;
pub(crate) mod tables;
pub mod world;
