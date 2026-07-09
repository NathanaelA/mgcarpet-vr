//! MC1 level-entity semantics: how a THING record's (class, model)
//! selects the entity's *type index* — the row in
//! [`crate::mc1::sprite_stats::SPRITE_STATS`] that drives its billboard
//! (and later, stats). Traced from the remc1 decompilation's per-model
//! spawn functions (`dword_96902` class dispatch, sub_main.cpp:5041;
//! full trace notes in docs/ROADMAP.md "Billboarded sprites").
//!
//! Non-drawable models (player-start markers, spawner volumes, logic
//! entities) map to `None`. Multi-part creatures (worms) map to their
//! head part; the trailing body segments are a runtime-behavior
//! concern, not a placement one.

/// How the original's spawn function picks the type index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mc1TypePick {
    /// Fixed row.
    Const(u16),
    /// Spawn-LCG draw, low bit: even -> first, odd -> second
    /// (trees: 83/84 at ~50/50).
    RandomBit(u16, u16),
    /// Spawn-LCG draw, `r % 7`: < 4 -> first, else second
    /// (class 5 model 13: 217/218).
    RandomSevenSplit(u16, u16),
    /// Per-model spawn-counter parity in slot order; first spawn gets
    /// the first value (class 5 model 7: 199/85).
    AlternateByCount(u16, u16),
    /// Village-owned mana: type 77 normally, raw-overwritten to 280
    /// by the spawner when the THING's `swi_id` (original `data_12`)
    /// is >= 3 (class 12, all models).
    Mana,
}

/// Type-index selection for one (class, model); `None` = the model
/// creates no drawable world entity at load (markers, spawner volumes,
/// logic-only), or is out of scope for load-time placement (class 10
/// terrain features — consumed by `crate::mc1::features`; its model 45
/// building/castle entities are the entity track's multi-tile
/// structure case, not a billboard).
pub fn mc1_entity_type(class: u16, model: u16) -> Option<Mc1TypePick> {
    use Mc1TypePick::*;
    Some(match (class, model) {
        // Class 2 — scenery/vegetation.
        (2, 0) => RandomBit(83, 84), // tree, two variants
        (2, 1) => Const(79),
        (2, 2) => Const(39),
        (2, 3) => Const(270),
        (2, 4) | (2, 5) => Const(48), // differ only in behavior state
        // Class 3 — balloons/castle; models 4-11 are the 8 player-start
        // markers (no entity).
        (3, 0) | (3, 1) => Const(44), // the wizard on his carpet
        (3, 2) => Const(177),         // castle (shares class 10 m45's type)
        (3, 3) => Const(169),
        // Class 10 -- terrain-feature/effect events; model 34 is the
        // PORTAL vortex, the one class-10 model that stands drawable in
        // the world (spawn sub_3B300 loads sprite row 223).
        (10, 34) => Const(223),
        // Class 5 — creatures. Multi-part worms map to their heads.
        (5, 0) => Const(40), // worm head (+ segments 19..=34 at runtime)
        (5, 1) => Const(86),
        (5, 2) => Const(3),
        (5, 3) => Const(88), // worm variant head (+ segments 89..=104)
        (5, 4) | (5, 15) => Const(0),
        (5, 5) => Const(185), // growth creature; 185..=192 by life ratio
        (5, 6) => Const(49),  // + parts 50, 193
        (5, 7) => AlternateByCount(199, 85),
        (5, 8) => Const(47),
        (5, 9) => Const(220),
        (5, 10) => Const(208),
        (5, 11) => Const(200),
        (5, 12) => Const(221),
        (5, 13) => RandomSevenSplit(217, 218),
        (5, 14) => Const(219),
        (5, 16) => Const(207),
        // Class 9 — spell/effect entities (not present in level files;
        // kept for the runtime track).
        (9, 0) | (9, 16) | (9, 18) | (9, 19) => Const(42),
        (9, 1) | (9, 17) => Const(209),
        (9, 2) | (9, 5) => Const(211),
        (9, 3) => Const(76),
        (9, 4) => Const(210),
        (9, 6) => Const(212),
        (9, 7) => Const(213),
        (9, 8) => Const(214),
        (9, 9) | (9, 12) => Const(216),
        (9, 10) => Const(18),
        (9, 11) => Const(281),
        (9, 13) => Const(195),
        (9, 14) => Const(196),
        (9, 15) => Const(215),
        // Class 12 — mana balls (village-owned when swi_id >= 3).
        (12, 0..=23) => Mana,
        // Everything else: markers, spawner volumes (class 11), logic
        // entities (class 1/6/7/8/13), terrain features (class 10).
        _ => return None,
    })
}

/// Trailing body parts of the multi-part creatures (type indices into
/// [`crate::mc1::sprite_stats::SPRITE_STATS`], spawn order preserved).
/// The original spawns them stacked on the head (state 120,
/// parent/child-linked; sub_38030 :44570) and its movement code
/// strings them out behind it from the first tick on.
pub fn mc1_entity_parts(class: u16, model: u16) -> &'static [u16] {
    match (class, model) {
        (5, 0) => &[
            19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
        ],
        (5, 3) => &[
            89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104,
        ],
        (5, 6) => &[50, 193],
        _ => &[],
    }
}

/// The spawn LCG (the engine's global `9377 * x + 9439` stream). At
/// load the original draws it in strict slot order across all spawn
/// functions; until entity spawning is ported 1:1, callers seed
/// per-slot for a stable approximation of the original's mix.
#[derive(Debug, Clone, Copy)]
pub struct SpawnRng(pub u32);

impl SpawnRng {
    pub fn draw(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(9377).wrapping_add(9439);
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_classes_resolve() {
        // The (class, model) pairs present across all baked MC1+HW
        // levels either resolve to a pick or are known non-drawables.
        assert_eq!(mc1_entity_type(2, 0), Some(Mc1TypePick::RandomBit(83, 84)));
        assert_eq!(mc1_entity_type(12, 7), Some(Mc1TypePick::Mana));
        assert_eq!(
            mc1_entity_type(5, 7),
            Some(Mc1TypePick::AlternateByCount(199, 85))
        );
        for m in 4..=11 {
            assert_eq!(mc1_entity_type(3, m), None, "player start m{m}");
        }
        assert_eq!(mc1_entity_type(11, 0), None, "spawner volume");
        assert_eq!(mc1_entity_type(10, 45), None, "building = entity track");
    }

    #[test]
    fn type_indices_fit_the_stats_table() {
        use crate::mc1::sprite_stats::SPRITE_STATS;
        for class in 0..16u16 {
            for model in 0..64u16 {
                let Some(pick) = mc1_entity_type(class, model) else {
                    continue;
                };
                let indices: Vec<u16> = match pick {
                    Mc1TypePick::Const(a) => vec![a],
                    Mc1TypePick::RandomBit(a, b)
                    | Mc1TypePick::RandomSevenSplit(a, b)
                    | Mc1TypePick::AlternateByCount(a, b) => vec![a, b],
                    Mc1TypePick::Mana => vec![77, 280],
                };
                for i in indices {
                    assert!(
                        (i as usize) < SPRITE_STATS.len(),
                        "class {class} model {model}: type {i} out of range"
                    );
                }
            }
        }
    }
}
