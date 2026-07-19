//! The shared chassis runtime — the game-agnostic engine that all
//! three games (MC1, Hidden Worlds, MC2) run on top of.
//!
//! This is a verbatim port of the remc1 core machinery, split from the
//! MC1-specific columns in [`crate::mc1`]. [`world::World`] is the
//! living-level runtime (trigger volumes, dispositions, spawned
//! entities, terrain-mutating events) and [`features::Gen`] is the
//! low-level engine state (the shared entity pool and load-time
//! GenerateFeatures pass). The game-specific spawn tables, spell
//! columns and rosters plug into this chassis from `mc1` and `mc2`.

pub mod features;
pub mod world;
