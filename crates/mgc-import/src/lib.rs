//! The importer/baker: the only code in the project that understands
//! original Bullfrog data formats.
//!
//! Everything here runs at import time, once per machine. Output is baked
//! packages (`mgc-formats`) that the engine consumes without knowing
//! anything about RNC, DAT/TAB, seeds, or XMI.
//!
//! Reference implementations: remc2 (`~/projects/remc2`) for MC2 behavior,
//! Moburma's tools and michaelhoward's MagicCarpetFileFormat spec for MC1
//! formats.

pub mod rnc;
