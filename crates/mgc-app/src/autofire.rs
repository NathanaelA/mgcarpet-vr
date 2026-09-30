//! The AUTOFIRE input macro (`controls.preferences.autofire`,
//! player-set 2026-09-30 — a deliberate deviation neither original
//! has; on under the Enhanced controls preset, off under Classic).
//!
//! Holding a fire button on a click-only projectile spell re-issues
//! the click for you, slowly: one cast per [`PERIOD`] ticks — 4 Hz,
//! half of what a quick finger does and a sixth of Rapid Fireball. It
//! is there to spare fingers and mouse switches, not to out-shoot
//! them; a real click always goes through at once, so clicking faster
//! still fires faster.
//!
//! It is purely an INPUT shaper: the sim receives ordinary one-tick
//! presses in `FlightInput::fire_left/right`, a recording tapes them
//! as clicks and a replay never runs this at all. What the hand holds
//! — and whether a click now would land — is the sim's read-only
//! [`AutofireHand`].
//!
//! The chain is started by a physical press and BROKEN by anything
//! that takes the cast away (release, death, the level ending, a new
//! level, a spell that is not on the list): it then stays silent until
//! the next physical press, however long the button is kept down.

use mgc_sim::engine::world::autofire::AutofireHand;

/// Ticks between autofire clicks: 24 Hz / 6 = 4 casts a second.
pub const PERIOD: u8 = 6;

/// One hand's click generator.
#[derive(Debug, Default, Clone, Copy)]
struct Pulser {
    /// A physical press started a chain nothing has broken yet.
    chain: bool,
    /// Ticks until the chain may click again.
    cooldown: u8,
    /// Last tick's physical level — the press detector.
    prev_held: bool,
}

impl Pulser {
    /// This tick's fire level for the sim, from the button's physical
    /// level and what the hand holds.
    fn tick(&mut self, held: bool, hand: AutofireHand) -> bool {
        let press = held && !self.prev_held;
        self.prev_held = held;
        self.cooldown = self.cooldown.saturating_sub(1);
        if !held {
            self.chain = false;
            return false;
        }
        match hand {
            // Not ours: the raw level, exactly as without the macro.
            AutofireHand::Manual => {
                self.chain = false;
                true
            }
            AutofireHand::Halted => {
                self.chain = false;
                press
            }
            AutofireHand::Wait | AutofireHand::Ready => {
                if press {
                    // A real click is never held back or rate-limited.
                    self.chain = true;
                    self.cooldown = PERIOD;
                    true
                } else if self.chain && self.cooldown == 0 && hand == AutofireHand::Ready {
                    self.cooldown = PERIOD;
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// Both hands' generators plus the level-over latch.
#[derive(Debug, Default, Clone, Copy)]
pub struct Autofire {
    hands: [Pulser; 2],
    level_over: bool,
}

impl Autofire {
    /// Break both chains (a new level, the option switched off): a
    /// button still held stays silent until it is pressed again.
    pub fn halt(&mut self) {
        for h in &mut self.hands {
            h.chain = false;
        }
    }

    /// Shape one tick's (left, right) fire levels. `level_over` is the
    /// level's completion latch — its rising edge breaks the chains
    /// once (MC1 keeps flying after the goal; a fresh click resumes).
    pub fn tick(
        &mut self,
        held: (bool, bool),
        hands: (AutofireHand, AutofireHand),
        level_over: bool,
    ) -> (bool, bool) {
        if level_over && !self.level_over {
            self.halt();
        }
        self.level_over = level_over;
        (
            self.hands[0].tick(held.0, hands.0),
            self.hands[1].tick(held.1, hands.1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AutofireHand::{Halted, Manual, Ready, Wait};

    /// Drive the left hand over a script of (held, hand) ticks.
    fn run(a: &mut Autofire, script: &[(bool, AutofireHand)]) -> Vec<bool> {
        script
            .iter()
            .map(|&(held, hand)| a.tick((held, false), (hand, Manual), false).0)
            .collect()
    }

    fn clicks(out: &[bool]) -> Vec<usize> {
        (0..out.len()).filter(|&i| out[i]).collect()
    }

    #[test]
    fn a_held_button_clicks_every_period() {
        let mut a = Autofire::default();
        let out = run(&mut a, &[(true, Ready); 20]);
        assert_eq!(clicks(&out), vec![0, 6, 12, 18]);
    }

    #[test]
    fn a_manual_spell_gets_the_raw_level() {
        let mut a = Autofire::default();
        let out = run(&mut a, &[(true, Manual); 8]);
        assert_eq!(out, vec![true; 8]);
    }

    #[test]
    fn a_real_click_always_goes_through_and_restarts_the_clock() {
        let mut a = Autofire::default();
        let mut script = vec![(true, Ready), (false, Ready), (true, Ready)];
        script.extend([(true, Ready); 7]);
        let out = run(&mut a, &script);
        assert_eq!(clicks(&out), vec![0, 2, 8]);
    }

    #[test]
    fn a_closed_gate_is_waited_out_and_fires_on_the_first_open_tick() {
        let mut a = Autofire::default();
        // A 9-tick cadence gate: closed at the 6-tick mark, open at 9.
        let mut script = vec![(true, Ready)];
        script.extend([(true, Wait); 8]);
        script.extend([(true, Ready); 3]);
        let out = run(&mut a, &script);
        assert_eq!(clicks(&out), vec![0, 9]);
        // A dry purse the same: the press itself still goes out (the
        // game's own refusal is the feedback), then silence until the
        // mana is back.
        let mut a = Autofire::default();
        let mut script = vec![(true, Wait); 30];
        script.extend([(true, Ready); 2]);
        let out = run(&mut a, &script);
        assert_eq!(clicks(&out), vec![0, 30]);
    }

    #[test]
    fn death_breaks_the_chain_until_the_next_press() {
        let mut a = Autofire::default();
        let mut script = vec![(true, Ready); 3];
        script.extend([(true, Halted); 4]);
        script.extend([(true, Ready); 20]); // respawned, still held
        script.extend([(false, Ready), (true, Ready)]);
        script.extend([(true, Ready); 6]);
        let out = run(&mut a, &script);
        assert_eq!(clicks(&out), vec![0, 28, 34]);
    }

    #[test]
    fn the_level_ending_breaks_the_chain_once() {
        let mut a = Autofire::default();
        let mut out = Vec::new();
        for t in 0..40 {
            // Goal reached at t=3; released at 20, pressed again at 21.
            out.push(a.tick((t != 20, false), (Ready, Manual), t >= 3).0);
        }
        assert_eq!(clicks(&out), vec![0, 21, 27, 33, 39]);
    }

    #[test]
    fn a_new_level_breaks_the_chain() {
        let mut a = Autofire::default();
        run(&mut a, &[(true, Ready); 3]);
        a.halt();
        let out = run(&mut a, &[(true, Ready); 20]);
        assert_eq!(clicks(&out), Vec::<usize>::new());
    }

    #[test]
    fn a_spell_swapped_in_under_a_held_button_does_not_start_a_chain() {
        let mut a = Autofire::default();
        let mut script = vec![(true, Manual); 3];
        script.extend([(true, Ready); 20]);
        let out = run(&mut a, &script);
        assert_eq!(clicks(&out), vec![0, 1, 2]);
    }

    #[test]
    fn the_hands_run_independent_clocks() {
        let mut a = Autofire::default();
        let mut l = Vec::new();
        let mut r = Vec::new();
        for t in 0..14 {
            let o = a.tick((true, t >= 2), (Ready, Ready), false);
            l.push(o.0);
            r.push(o.1);
        }
        assert_eq!(clicks(&l), vec![0, 6, 12]);
        assert_eq!(clicks(&r), vec![2, 8]);
    }
}
