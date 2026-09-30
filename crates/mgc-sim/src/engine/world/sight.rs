//! BEYOND SIGHT'S TRIGGER AREAS — the sim's read-only half of the
//! app's `render.enhancement.map_beyond_sight_areas` map ripple
//! (player-set 2026-09-30, a deliberate deviation: neither original
//! reveals trigger areas, and until now only the debug overlay behind
//! [`World::active_volumes`] did).
//!
//! What is revealed is the set of FLY-INTO volumes: the class-11
//! proximity triggers, one-shot or repeating, enter or leave polarity.
//! The triggers that are not a place — the kill-watchers, the win /
//! level-end releases, MC2's stage-gated switches — are left out by
//! ruling, and so are the things the map already shows on its own
//! (portals, MC2's ending markers and objective checkpoints).
//!
//! Each area carries one verdict, TRAP or not, judged on what it would
//! fire right now: the live THING rows whose `dis_id` is the trigger's
//! disposition — exactly the set [`World::fire_disposition`] would
//! spawn. A trap is a payload that comes for the player: something
//! that burns, blasts, strikes, robs or hunts. Everything else — mana,
//! spell jars, scenery, terrain work, further switches, harmless folk —
//! is an effector. A volume with nothing left behind it is not shown.
//!
//! A pure read: no state, nothing hashed, nothing snapshotted.

use super::World;
use crate::ids::GameId;

/// One fly-into trigger area, in tile units (`radius` = the volume's
/// half-extent; the volume itself is an axis-aligned box).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightArea {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    /// The payload comes for the player (see the module note).
    pub trap: bool,
}

/// MC1 / Hidden Worlds payload rows that make a trigger a TRAP.
///
/// Creatures (class 5): every hunter. The Builder (12), Townie (13)
/// and Trader (14) never attack, and the Archers (4) are militia —
/// they only engage a wizard the village already wants.
///
/// Effects (class 10), the ones whose area write reaches the player
/// (a trigger-spawned effect has no owner, so nothing exempts him):
/// 0 fire / explosion, 1 its spreader, 6 standing fire, 9 volcano,
/// 11 crater, 15 earthquake, 17 meteor, 23 lightning, 25 steal mana —
/// and 52, the egg that hatches a wild crab. Not traps: splash and
/// smoke (5/13/14), the one-tick mini volcano (8), the stubbed rain of
/// fire (24) and null 49, teleports (34), mana (39), dwellings (45).
fn mc1_trap(class: u16, model: u16) -> bool {
    match class {
        5 => !matches!(model, 4 | 12..=14),
        10 => matches!(model, 0 | 1 | 6 | 9 | 11 | 15 | 17 | 23 | 25 | 52),
        _ => false,
    }
}

/// MC2 payload rows that make a trigger a TRAP — the same rule on
/// MC2's own model numbers.
///
/// Creatures: everything but the goat (1), the archers (4, militia as
/// in MC1), the builder (12), the villager (13) and the trader (14).
///
/// Effects: 0 fire cell, 1 big explosion, 6 standing fire, 9 the
/// raise-land dome's damage pulse, 11 scorch ring / lava pool, 15 the
/// quake's fire trail, 17 meteor, 22 whirlwind, 23 lightning, 25 steal
/// mana, 57 FOOL'S MANA (the bait sphere that answers a claim with a
/// fireball), 67 the flood dome, 71 fissure, 76 the fire-sphere orb.
/// Not traps: splash / smoke (5/13/14/59/60/86), teleporter (34), true
/// mana (39/58), buildings and their anchor (45/52), the mana-magnet
/// aura (54), the risers (63/64) and the cave sculptors (83-85).
fn mc2_trap(class: u16, model: u16) -> bool {
    match class {
        5 => !matches!(model, 1 | 4 | 12..=14),
        10 => matches!(
            model,
            0 | 1 | 6 | 9 | 11 | 15 | 17 | 22 | 23 | 25 | 57 | 67 | 71 | 76
        ),
        _ => false,
    }
}

impl World {
    /// The fly-into trigger areas and their trap verdicts — what
    /// Beyond Sight reveals on the map surfaces.
    pub fn sight_areas(&self) -> Vec<SightArea> {
        let mc2 = matches!(self.game, GameId::Mc2);
        let mut out = Vec::new();
        for e in &self.g.ent {
            if e.class64 != 11 || e.flags & 0x400 != 0 {
                continue;
            }
            // The proximity family: both games' 0..=3, and MC1's two
            // further enter/leave × one-shot/repeating quartets. MC2's
            // 12 and 31 are the ending markers, not switches.
            let proximity = if mc2 {
                matches!(e.tick70, 0..=3)
            } else {
                matches!(e.tick70, 0..=3 | 5..=12)
            };
            if !proximity {
                continue;
            }
            let Some(trap) = self.disposition_verdict(e.id24) else {
                continue;
            };
            out.push(SightArea {
                x: e.x as f32 / 256.0,
                z: e.y as f32 / 256.0,
                radius: (e.f80 as f32 / 256.0).max(0.5),
                trap,
            });
        }
        out
    }

    /// What firing disposition `dis` would do right now: `None` =
    /// nothing at all (no live row, no StageVar keyed to it),
    /// `Some(true)` = at least one row is a trap payload.
    fn disposition_verdict(&self, dis: u16) -> Option<bool> {
        let mc2 = matches!(self.game, GameId::Mc2);
        let mut any = mc2
            && self
                .mc2_stagevars
                .iter()
                .any(|v| v.kind == 7 && v.param == dis);
        for r in self.table.iter().skip(1) {
            if r.class == 0 || r.dis_id != dis {
                continue;
            }
            any = true;
            if if mc2 {
                mc2_trap(r.class, r.model)
            } else {
                mc1_trap(r.class, r.model)
            } {
                return Some(true);
            }
        }
        any.then_some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::features::{FeatureAssets, Planes};
    use crate::engine::world::{PlayerCommand, PlayerPose};
    use mgc_formats::{Thing, ThingKind};

    fn planes() -> Planes {
        Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        }
    }

    fn assets() -> FeatureAssets {
        FeatureAssets {
            rings: (0..32).map(|_| vec![(15u8, 15u8)]).collect(),
            build_tab: Vec::new(),
            build_dat: Vec::new(),
            bldgprm: Vec::new(),
            spells: Vec::new(),
            mc2_sprite_ext: Vec::new(),
            mc1_sprite_ext: Vec::new(),
        }
    }

    /// (class, model, x, y, dis_id, swi_sz, swi_id) rows, slots 1...
    fn things(rows: &[(u16, u16, u16, u16, u16, u16, u16)]) -> Vec<Thing> {
        rows.iter()
            .enumerate()
            .map(|(i, &(class, model, x, y, dis_id, swi_sz, swi_id))| Thing {
                slot: i as u32 + 1,
                kind: ThingKind::Entity,
                class,
                model,
                x,
                y,
                dis_id,
                swi_sz,
                swi_id,
                parent: 0,
                child: 0,
                par3: None,
            })
            .collect()
    }

    fn world(game: GameId, rows: &[(u16, u16, u16, u16, u16, u16, u16)]) -> World {
        World::new_for_game(planes(), &things(rows), 1, assets(), game)
    }

    /// The area whose centre tile is (x, y).
    fn at(areas: &[SightArea], x: u16, y: u16) -> Option<SightArea> {
        areas
            .iter()
            .copied()
            .find(|a| a.x.floor() as u16 == x && a.z.floor() as u16 == y)
    }

    #[test]
    fn mc1_areas_are_the_fly_into_triggers_judged_on_their_payload() {
        let w = world(
            GameId::Mc1,
            &[
                // Enter one-shot → an explosion: a trap.
                (11, 0, 50, 50, 0, 4, 7),
                (10, 0, 52, 52, 7, 0, 0),
                // Repeating → a spell jar, a standing stone and a
                // dwelling: an effector.
                (11, 2, 80, 80, 0, 6, 8),
                (12, 5, 81, 81, 8, 0, 0),
                (2, 1, 82, 82, 8, 0, 0),
                (10, 45, 83, 83, 8, 0, 0),
                // Leave one-shot → townies and archers (militia), and
                // one dragon among them.
                (11, 1, 120, 40, 0, 3, 9),
                (5, 13, 121, 41, 9, 0, 0),
                (5, 4, 121, 42, 9, 0, 0),
                (5, 0, 122, 41, 9, 0, 0),
                // Nothing behind it: not shown.
                (11, 0, 150, 150, 0, 5, 10),
                // A kill-watcher is not a place, whatever it fires.
                (11, 13, 170, 170, 0, 5, 7),
                // MC1's second enter quartet counts too.
                (11, 5, 200, 60, 0, 2, 11),
                (10, 39, 200, 61, 11, 0, 0),
            ],
        );
        let areas = w.sight_areas();
        assert_eq!(areas.len(), 4, "{areas:?}");
        let a = at(&areas, 50, 50).expect("the explosion trigger");
        assert!(a.trap);
        assert_eq!(a.radius, 4.0);
        assert!(!at(&areas, 80, 80).expect("the jar trigger").trap);
        assert!(at(&areas, 120, 40).expect("the ambush").trap);
        assert!(!at(&areas, 200, 60).expect("the mana trigger").trap);
    }

    #[test]
    fn mc2_areas_use_mc2s_own_models() {
        let w = world(
            GameId::Mc2,
            &[
                // Fool's mana is bait: a trap.
                (11, 0, 50, 50, 0, 4, 7),
                (10, 57, 52, 52, 7, 0, 0),
                // True mana and a riser: an effector.
                (11, 2, 80, 80, 0, 6, 8),
                (10, 58, 81, 81, 8, 0, 0),
                (10, 64, 82, 82, 8, 0, 0),
                // MC2's 45 is a building, its goats are harmless.
                (11, 1, 120, 40, 0, 3, 9),
                (10, 45, 121, 41, 9, 0, 0),
                (5, 1, 122, 41, 9, 0, 0),
                // A hydra is not.
                (11, 3, 160, 40, 0, 3, 12),
                (5, 27, 161, 41, 12, 0, 0),
                // The ending marker and the stage-gated switch are not
                // fly-into areas; MC2 has no 5..=12 proximity quartets.
                (11, 12, 200, 60, 0, 2, 7),
                (11, 32, 210, 60, 0, 2, 7),
                (11, 5, 220, 60, 0, 2, 7),
            ],
        );
        let areas = w.sight_areas();
        assert_eq!(areas.len(), 4, "{areas:?}");
        assert!(at(&areas, 50, 50).expect("fool's mana").trap);
        assert!(!at(&areas, 80, 80).expect("mana + riser").trap);
        assert!(!at(&areas, 120, 40).expect("village").trap);
        assert!(at(&areas, 160, 40).expect("hydra").trap);
    }

    #[test]
    fn a_fired_one_shot_leaves_the_map() {
        let mut w = world(
            GameId::Mc1,
            &[(11, 0, 50, 50, 0, 4, 7), (10, 13, 52, 52, 7, 0, 0)],
        );
        assert_eq!(w.sight_areas().len(), 1);
        let inside = PlayerPose::from_tiles(50.5, 100.0 / 8.0 + 1.0, 50.5, 0.0, 0.0, 0.0);
        for _ in 0..16 {
            w.tick(inside, PlayerCommand::default());
        }
        assert!(w.sight_areas().is_empty(), "fired, consumed, gone");
    }
}
