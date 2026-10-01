//! The Beyond-Sight trigger areas as the map shows them
//! (`render.enhancement.map_beyond_sight_areas`): the sim's live set,
//! with a per-area fade so nothing pops. The whole effect already
//! swells and settles with the spell (`App::sight_strength`); this is
//! the part for a SINGLE area — a trigger that fires while the spell
//! is up settles out over the same [`FADE_SECS`] instead of vanishing,
//! and one that comes into being mid-spell (a chained switch) swells
//! in.

use mgc_render::SightArea;

/// Seconds the ripple takes to swell when the spell goes up or an area
/// appears, and to settle when the spell lapses or a trigger fires.
pub const FADE_SECS: f32 = 1.2;

/// One shown area: `area.weight` is its eased density, `live` whether
/// its trigger still stands.
#[derive(Debug, Clone, Copy)]
struct Fade {
    area: SightArea,
    live: bool,
}

/// The areas on show, fading ones included.
#[derive(Debug, Clone, Default)]
pub struct SightAreas(Vec<Fade>);

/// The same trigger as far as the map can tell (weight aside).
fn same(a: &SightArea, b: &SightArea) -> bool {
    a.x == b.x && a.z == b.z && a.radius == b.radius && a.trap == b.trap
}

impl SightAreas {
    /// Take the sim's live set for this tick. `unseen` = nothing is on
    /// screen (the spell is only now coming up): the set is adopted
    /// whole at full weight and the global swell does the fading.
    /// Otherwise areas that left start settling and new ones start
    /// from nothing.
    pub fn merge(&mut self, now: Vec<SightArea>, unseen: bool) {
        if unseen {
            self.0 = now
                .into_iter()
                .map(|a| Fade {
                    area: SightArea { weight: 1.0, ..a },
                    live: true,
                })
                .collect();
            return;
        }
        for f in &mut self.0 {
            f.live = false;
        }
        for a in now {
            match self.0.iter_mut().find(|f| same(&f.area, &a)) {
                Some(f) => f.live = true,
                None => self.0.push(Fade {
                    area: SightArea { weight: 0.0, ..a },
                    live: true,
                }),
            }
        }
    }

    /// Advance the per-area fades by one frame of `dt` seconds.
    pub fn ease(&mut self, dt: f32) {
        let step = dt / FADE_SECS;
        for f in &mut self.0 {
            f.area.weight = if f.live {
                (f.area.weight + step).min(1.0)
            } else {
                (f.area.weight - step).max(0.0)
            };
        }
        self.0.retain(|f| f.live || f.area.weight > 0.0);
    }

    /// What the renderer draws.
    pub fn areas(&self) -> Vec<SightArea> {
        self.0.iter().map(|f| f.area).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: f32, trap: bool) -> SightArea {
        SightArea {
            x,
            z: 10.0,
            radius: 4.0,
            trap,
            weight: 1.0,
        }
    }

    fn weights(s: &SightAreas) -> Vec<f32> {
        s.areas().iter().map(|a| a.weight).collect()
    }

    #[test]
    fn a_spell_coming_up_adopts_the_set_at_full_weight() {
        let mut s = SightAreas::default();
        s.merge(vec![area(1.0, true), area(2.0, false)], true);
        assert_eq!(weights(&s), vec![1.0, 1.0]);
        // …and replaces whatever a past cast left behind.
        s.merge(vec![area(3.0, true)], true);
        assert_eq!(s.areas().len(), 1);
        assert_eq!(s.areas()[0].x, 3.0);
    }

    #[test]
    fn a_fired_trigger_settles_out_instead_of_vanishing() {
        let mut s = SightAreas::default();
        s.merge(vec![area(1.0, true), area(2.0, false)], true);
        // The trap fires: the sim no longer reports it.
        s.merge(vec![area(2.0, false)], false);
        assert_eq!(weights(&s), vec![1.0, 1.0], "still there this frame");
        s.ease(FADE_SECS / 2.0);
        let w = weights(&s);
        assert!((w[0] - 0.5).abs() < 1e-5 && w[1] == 1.0, "{w:?}");
        s.ease(FADE_SECS);
        assert_eq!(s.areas().len(), 1, "gone once it has settled");
        assert_eq!(s.areas()[0].x, 2.0);
    }

    #[test]
    fn an_area_born_mid_spell_swells_in() {
        let mut s = SightAreas::default();
        s.merge(vec![area(1.0, true)], true);
        s.merge(vec![area(1.0, true), area(5.0, false)], false);
        assert_eq!(weights(&s), vec![1.0, 0.0]);
        s.ease(FADE_SECS / 4.0);
        assert!((weights(&s)[1] - 0.25).abs() < 1e-5);
        // A steady set stays put, and a verdict flip cross-fades.
        s.merge(vec![area(1.0, false), area(5.0, false)], false);
        s.ease(FADE_SECS / 4.0);
        let w = weights(&s);
        assert_eq!(w.len(), 3, "old red settling, new cyan swelling: {w:?}");
        assert!(
            (w[0] - 0.75).abs() < 1e-5 && (w[2] - 0.25).abs() < 1e-5,
            "{w:?}"
        );
    }
}
