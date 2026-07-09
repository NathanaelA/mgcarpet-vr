//! The MC2 (Magic Carpet 2: The Netherworlds) simulation column —
//! tier-2 tables and (as Phase 3 lands them) the tier-4 handlers and
//! tier-5 verb arms behind [`crate::verbs`]. Everything here is a
//! verbatim port of remc2 machinery; shared chassis stays in
//! [`crate::mc1::features::Gen`] per the Phase-0 survey (same pool,
//! LCG, mailboxes, tile chains).
//!
//! Data provenance: `behavior.rs` + `sprite_params.rs` are generated
//! by `tools/extract-remc2-tables.py` from the vendored remc2
//! decompilation; the per-level building parameters (`bldgprm.bin`),
//! ring table (`search.bin`) and spell table (`spells.bin`) ride the
//! mc2-* asset bundles.

pub mod behavior;
pub(crate) mod doomsday;
pub(crate) mod effects;
pub(crate) mod mobs;
pub(crate) mod morph;
pub(crate) mod multipart;
pub(crate) mod proj;
pub(crate) mod riser;
pub(crate) mod roster;
pub(crate) mod scenery;
pub mod sin_lut;
pub mod spells;
pub mod sprite_params;
pub(crate) mod tail;
pub mod terrain_paint;
pub(crate) mod tokens;

#[cfg(test)]
mod tests {
    use super::behavior::{BEHAVIOR, Mc2BehaviorRow, ROW_BASE};

    /// The Phase-0 survey's cross-engine anchor: MC2's model-0 row
    /// (array index 59, the engine's base pointer) is byte-identical
    /// to MC1's BEHAVIOR[0] — proven here against both extractions.
    #[test]
    fn mc2_model0_row_matches_mc1_row0() {
        let m2 = &BEHAVIOR[ROW_BASE];
        let m1 = &crate::mc1::behavior::BEHAVIOR[0];
        assert_eq!(
            (
                m2.v_0, m2.v_2, m2.v_4, m2.v_6, m2.v_8, m2.v_10, m2.v_12, m2.v_14
            ),
            (
                m1.v_0, m1.v_2, m1.v_4, m1.v_6, m1.v_8, m1.v_10, m1.v_12, m1.v_14
            )
        );
        assert_eq!(
            (
                m2.v_16, m2.v_18, m2.v_20, m2.v_22, m2.v_26, m2.v_28, m2.v_30
            ),
            (
                m1.v_16, m1.v_18, m1.v_20, m1.v_22, m1.v_26, m1.v_28, m1.v_30
            )
        );
        assert_eq!(m2.flags, 0);
    }

    /// The slice creatures' hand-picked rows (ctors assign ABSOLUTE
    /// indices — remc2 :33739/:33899/:34058) and their flag bytes:
    /// Goat + Villager flee (bit 8) and die on water (bit 1);
    /// Archers only die on water. Nobody disables the pack scan.
    #[test]
    fn slice_rows_and_flags() {
        let goat = &BEHAVIOR[98];
        let archers = &BEHAVIOR[75];
        let villager = &BEHAVIOR[100];
        assert_eq!((goat.v_0, goat.flags), (0x27, 0x09));
        assert_eq!((archers.v_0, archers.flags), (0x10, 0x01));
        assert_eq!((villager.v_0, villager.flags), (0x29, 0x09));
        assert_eq!(
            goat.flags & Mc2BehaviorRow::FLEE,
            Mc2BehaviorRow::FLEE,
            "goat flees"
        );
        assert_eq!(archers.flags & Mc2BehaviorRow::FLEE, 0, "archers hold");
        for r in [goat, archers, villager] {
            assert_eq!(
                r.flags & Mc2BehaviorRow::PACK_DISABLE,
                0,
                "no slice model disables the pack scan"
            );
        }
    }
}
