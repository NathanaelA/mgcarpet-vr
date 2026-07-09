//! Faithful port of MC1's sound mixer (remc1, all citations
//! sub_main.cpp):
//!
//! - Request phase `sub_55370_558A0` (:64444): every sim sound event
//!   lands in ONE pending slot per sound id per tick, loudest request
//!   winning within an 8-unit tolerance (`sub_55870`: replace unless
//!   `new < old - 8`). Volume = linear falloff over a range that
//!   shrinks for sounds behind the listener (12288 world units ahead,
//!   9216 behind; cull beyond 12288² squared distance, drop below
//!   512); pan engages only beyond 320 units — closer sounds sit
//!   center.
//! - Flush phase `sub_55100_55630` (:64459): once per tick the 47
//!   slots issue channel operations. Mode 1 (`sub_483C0`) = restart:
//!   an already-running instance with the same (tag, id) is stopped
//!   first. Mode 3 (`sub_48470`) = don't-interrupt: the request is
//!   dropped while a (tag, id) instance still runs. 32 driver
//!   channels (`word_CBFF0`), free-channel allocation, no stealing
//!   (`sub_48570` silently drops when all 32 are busy).
//! - Ambient loops `sub_520F0`/`sub_52120`/`sub_52400` + fade pumps
//!   `sub_51FC0`/`sub_522E0`: waves (1) XOR wind (2) switched by the
//!   terrain under the player, fire (5) and market (31) by proximity;
//!   loop targets 70/120/85 (<<8), fade-in +2048/tick, fade-out
//!   -2048/tick with the channel cut below 4096.
//!
//! Positions are engine units (wrapping u16 torus, 256 per tile),
//! yaw the sim's 0..2047 sine-table angle.
//! The enhanced distance-weighted emitter mixer (authenticity matrix)
//! will sit beside this as a sibling implementation feeding the same
//! output backend.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::output::{CHANNELS, Cmd};

/// Sound ids (bank 0) with engine-defined mixing behavior.
pub const SND_WAVES: u8 = 1;
pub const SND_WIND: u8 = 2;
pub const SND_FIRE_AMBIENT: u8 = 5;
pub const SND_MARKET: u8 = 31;

const SLOT_COUNT: usize = 47;
const CULL_DIST_SQ: i64 = 150_994_944; // 12288^2
const BASE_RANGE: i32 = 12288;
const MIN_VOL: i32 = 512;
const PAN_MIN_DIST: i32 = 320;
const FADE_STEP: i32 = 2048;
const FADE_FLOOR: i32 = 4096;

/// Per-id flush behavior (the `sub_55370` switch).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Policy {
    /// Mode 1: restart a running (tag, id) instance.
    Restart,
    /// Mode 3: keep a running (tag, id) instance, drop the request.
    KeepRunning,
    /// Modes 1/3 but only accepted for player-sourced (or untagged)
    /// requests (ids 4, 14, 29 / 17).
    RestartPlayerOnly,
    KeepRunningPlayerOnly,
    /// Ambient loop with a fixed fade-in target (ids 1, 2, 5, 31).
    Loop(u16),
    /// Everything else: the original's default case drops it.
    Drop,
}

fn policy(id: u8) -> Policy {
    match id {
        1 | 2 => Policy::Loop(70),
        5 => Policy::Loop(120),
        31 => Policy::Loop(85),
        3 | 9 | 15 | 16 | 18..=28 | 30 | 40 | 43..=45 => Policy::Restart,
        4 | 14 | 29 => Policy::RestartPlayerOnly,
        7 | 8 | 10..=13 | 32..=39 | 41 => Policy::KeepRunning,
        17 => Policy::KeepRunningPlayerOnly,
        _ => Policy::Drop,
    }
}

/// Where a sound comes from, for attenuation and instance tagging.
/// Positions are engine units on the wrapping u16 torus (256 units
/// per tile), matching the sim's entity coordinates.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// UI / player-own sounds: full volume, center pan.
    Player,
    /// World-positioned, tagged by the emitting entity so repeated
    /// requests restart the same instance (the original's
    /// entity+24 tag through `word_12CD26`).
    World { pos: (u16, u16, i16), tag: u16 },
}

/// Listener state for one tick.
#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub pos: (u16, u16, i16),
    /// Engine yaw, 0..2047, 0 = north (-y) — the sim's sine-table
    /// convention (mobs::polar_step).
    pub yaw: u16,
}

#[derive(Clone, Copy, Default)]
struct Slot {
    pending: bool,
    restart: bool,
    vol: u16,
    pan: u16,
    tag: u16,
}

#[derive(Clone, Copy, PartialEq)]
enum Fade {
    None,
    In { target: i32 },
    Out,
}

#[derive(Clone, Copy)]
struct Channel {
    /// (tag, id) occupying this channel; None = believed free.
    key: Option<(u16, u8)>,
    looped: bool,
    /// Volume on the 0..32767 scale (the original's CC070<<8 view).
    vol: i32,
    fade: Fade,
}

/// The sample bank as the runtime sees it: engine id → PCM.
pub struct Sounds {
    pub sample_rate: u32,
    /// Indexed by sound id; None where the bank has no entry.
    pub entries: Vec<Option<Arc<Vec<u8>>>>,
}

impl Sounds {
    /// Build from a loaded audio bundle's bank.
    pub fn from_bundle(bundle: &mgc_formats::bundle::AudioBundle, bank: u32) -> Option<Sounds> {
        let (index, blob) = bundle.sounds.as_ref()?;
        let bank = index.banks.iter().find(|b| b.bank == bank)?;
        let max_id = bank.entries.iter().map(|e| e.id).max().unwrap_or(0) as usize;
        let mut entries = vec![None; max_id + 1];
        for e in &bank.entries {
            let pcm = blob[e.offset as usize..(e.offset + e.len) as usize].to_vec();
            entries[e.id as usize] = Some(Arc::new(pcm));
        }
        Some(Sounds {
            sample_rate: index.sample_rate,
            entries,
        })
    }
}

pub struct FaithfulMixer {
    slots: [Slot; SLOT_COUNT],
    channels: [Channel; CHANNELS],
    /// Ambient wishes for this tick (set by the app from sim state).
    ambient: [bool; 4], // waves, wind, fire, market
}

impl Default for FaithfulMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl FaithfulMixer {
    pub fn new() -> FaithfulMixer {
        FaithfulMixer {
            slots: [Slot::default(); SLOT_COUNT],
            channels: [Channel {
                key: None,
                looped: false,
                vol: 0,
                fade: Fade::None,
            }; CHANNELS],
            ambient: [false; 4],
        }
    }

    /// The request phase: one sim sound event.
    pub fn request(&mut self, id: u8, source: Source, listener: &Listener) {
        if id as usize >= SLOT_COUNT {
            return;
        }
        let (vol, pan, tag, player_sourced) = match source {
            Source::Player => (0x7FFF_u16, 0x7FFF_u16, 0u16, true),
            Source::World { pos, tag } => {
                // Torus-wrapped deltas (the original's i16 truncation).
                let dx = i64::from(pos.0.wrapping_sub(listener.pos.0) as i16);
                let dy = i64::from(pos.1.wrapping_sub(listener.pos.1) as i16);
                let dz = i64::from(pos.2 - listener.pos.2);
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq > CULL_DIST_SQ {
                    return;
                }
                let dist = (dist_sq as f64).sqrt() as i32;
                // Horizontal bearing, engine 2048-space, 0 = north
                // (-y): forward = (sin yaw, -cos yaw).
                let bearing = ((dx as f64).atan2(-dy as f64) / std::f64::consts::TAU * 2048.0)
                    .rem_euclid(2048.0) as i32;
                // Angular offset from facing in the mixer's 1024
                // half-space (sub_42210), folded to 0..512.
                let off2048 = (bearing - i32::from(listener.yaw)).rem_euclid(2048);
                let right = off2048 < 1024; // sign before folding
                let off = {
                    let o = off2048 / 2;
                    if o > 512 { 1024 - o } else { o }
                };
                // Audible range shrinks behind the listener
                // (`12288 * (1024 - off/2) / 1024`).
                let range = BASE_RANGE * (1024 - off / 2) / 1024;
                let vol = 0x7FFF * (range - dist) / range.max(1);
                if vol < MIN_VOL {
                    return;
                }
                let vol = vol.min(0x7FFF) as u16;
                let pan = if dist > PAN_MIN_DIST {
                    let swing = off.min(1024 - off) << 6; // 0..32768
                    let p = 0x7FFF + if right { swing } else { -swing };
                    p.clamp(0, 0xFFFF) as u16
                } else {
                    0x7FFF
                };
                (vol, pan, tag, false)
            }
        };

        match policy(id) {
            Policy::Drop => {}
            Policy::Loop(_) => {} // loops are driven by ambient state
            p => {
                let slot = &mut self.slots[id as usize];
                // sub_55870: replace unless clearly quieter.
                if slot.pending && i32::from(vol) < i32::from(slot.vol) - 8 {
                    return;
                }
                let restart = matches!(p, Policy::Restart | Policy::RestartPlayerOnly);
                if matches!(p, Policy::RestartPlayerOnly | Policy::KeepRunningPlayerOnly)
                    && !player_sourced
                {
                    return;
                }
                *slot = Slot {
                    pending: true,
                    restart,
                    vol,
                    pan,
                    tag,
                };
            }
        }
    }

    /// Ambient wishes for this tick; waves/wind are exclusive in the
    /// original (water under the player switches between them).
    pub fn set_ambient(&mut self, waves: bool, fire: bool, market: bool) {
        self.ambient = [waves, !waves, fire, market];
    }

    /// The flush phase: run fades, sync loop states, issue channel
    /// commands. `live_mask` = the output's per-channel liveness.
    pub fn tick(&mut self, sounds: &Sounds, tx: &Sender<Cmd>, live_mask: u32) {
        // Reconcile one-shot channels that finished on their own.
        for (i, ch) in self.channels.iter_mut().enumerate() {
            if ch.key.is_some() && !ch.looped && live_mask & (1 << i) == 0 {
                ch.key = None;
                ch.fade = Fade::None;
            }
        }

        // Ambient loop wishes → fade state (sub_520F0 / sub_55890).
        let wishes = [
            (SND_WAVES, self.ambient[0], 70u16),
            (SND_WIND, self.ambient[1], 70),
            (SND_FIRE_AMBIENT, self.ambient[2], 120),
            (SND_MARKET, self.ambient[3], 85),
        ];
        for (id, want, target) in wishes {
            let running = self.find_channel(0, id);
            match (want, running) {
                (true, Some(i)) => {
                    let ch = &mut self.channels[i];
                    if ch.fade
                        != (Fade::In {
                            target: i32::from(target) << 8,
                        })
                    {
                        ch.fade = Fade::In {
                            target: i32::from(target) << 8,
                        };
                    }
                }
                (true, None) => {
                    if let Some(free) = self.free_channel(live_mask) {
                        if let Some(pcm) = sample(sounds, id) {
                            let _ = tx.send(Cmd::Play {
                                ch: free,
                                pcm,
                                sample_rate: sounds.sample_rate,
                                vol: 0,
                                pan: 0x7FFF,
                                looped: true,
                            });
                            self.channels[free] = Channel {
                                key: Some((0, id)),
                                looped: true,
                                vol: 0,
                                fade: Fade::In {
                                    target: i32::from(target) << 8,
                                },
                            };
                        }
                    }
                }
                (false, Some(i)) => {
                    if self.channels[i].fade != Fade::Out {
                        self.channels[i].fade = Fade::Out;
                    }
                }
                (false, None) => {}
            }
        }

        // Fade pumps (sub_51FC0 fade-in, sub_522E0 fade-out).
        for (i, ch) in self.channels.iter_mut().enumerate() {
            match ch.fade {
                Fade::In { target } => {
                    ch.vol = (ch.vol + FADE_STEP).min(0x7FFF).min(target - 1).max(0);
                    let _ = tx.send(Cmd::SetVol {
                        ch: i,
                        vol: ch.vol as u16,
                    });
                    if ch.vol >= target - 1 || ch.vol >= 0x7FFF {
                        ch.fade = Fade::None;
                    }
                }
                Fade::Out => {
                    ch.vol -= FADE_STEP;
                    if ch.vol > FADE_FLOOR {
                        let _ = tx.send(Cmd::SetVol {
                            ch: i,
                            vol: ch.vol as u16,
                        });
                    } else {
                        let _ = tx.send(Cmd::Stop { ch: i });
                        *ch = Channel {
                            key: None,
                            looped: false,
                            vol: 0,
                            fade: Fade::None,
                        };
                    }
                }
                Fade::None => {}
            }
        }

        // Slot flush (sub_55100).
        for id in 0..SLOT_COUNT as u8 {
            let slot = self.slots[id as usize];
            if !slot.pending {
                continue;
            }
            self.slots[id as usize] = Slot::default();
            let running = self.find_channel(slot.tag, id);
            match (slot.restart, running) {
                (false, Some(_)) => continue, // mode 3: keep running
                (true, Some(i)) => {
                    let _ = tx.send(Cmd::Stop { ch: i });
                    self.channels[i].key = None;
                }
                _ => {}
            }
            let Some(free) = self.free_channel(live_mask) else {
                continue; // all 32 busy: the original drops too
            };
            let Some(pcm) = sample(sounds, id) else {
                continue;
            };
            let _ = tx.send(Cmd::Play {
                ch: free,
                pcm,
                sample_rate: sounds.sample_rate,
                vol: slot.vol,
                pan: slot.pan,
                looped: false,
            });
            self.channels[free] = Channel {
                key: Some((slot.tag, id)),
                looped: false,
                vol: i32::from(slot.vol),
                fade: Fade::None,
            };
        }
    }

    fn find_channel(&self, tag: u16, id: u8) -> Option<usize> {
        self.channels.iter().position(|c| c.key == Some((tag, id)))
    }

    fn free_channel(&self, live_mask: u32) -> Option<usize> {
        self.channels
            .iter()
            .enumerate()
            .position(|(i, c)| c.key.is_none() && live_mask & (1 << i) == 0)
    }
}

fn sample(sounds: &Sounds, id: u8) -> Option<Arc<Vec<u8>>> {
    sounds.entries.get(id as usize)?.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sounds() -> Sounds {
        Sounds {
            sample_rate: 22050,
            entries: (0..SLOT_COUNT)
                .map(|_| Some(Arc::new(vec![128u8; 1000])))
                .collect(),
        }
    }

    fn listener() -> Listener {
        Listener {
            pos: (0, 0, 0),
            yaw: 0,
        }
    }

    #[test]
    fn distance_attenuates_and_culls() {
        let mut m = FaithfulMixer::new();
        // Beyond 12288: culled.
        m.request(
            9,
            Source::World {
                pos: (13000, 0, 0),
                tag: 1,
            },
            &listener(),
        );
        assert!(!m.slots[9].pending);
        // Close: loud.
        m.request(
            9,
            Source::World {
                pos: (1000, 0, 0),
                tag: 1,
            },
            &listener(),
        );
        assert!(m.slots[9].pending);
        assert!(m.slots[9].vol > 0x7000, "vol {}", m.slots[9].vol);
    }

    #[test]
    fn loudest_request_wins_slot() {
        let mut m = FaithfulMixer::new();
        let l = listener();
        m.request(
            9,
            Source::World {
                pos: (8000, 0, 0),
                tag: 1,
            },
            &l,
        );
        let quiet = m.slots[9].vol;
        m.request(
            9,
            Source::World {
                pos: (500, 0, 0),
                tag: 2,
            },
            &l,
        );
        assert!(m.slots[9].vol > quiet);
        assert_eq!(m.slots[9].tag, 2);
        // A far request must not displace the near one.
        m.request(
            9,
            Source::World {
                pos: (9000, 0, 0),
                tag: 3,
            },
            &l,
        );
        assert_eq!(m.slots[9].tag, 2);
    }

    #[test]
    fn near_sounds_center_far_sounds_pan() {
        let mut m = FaithfulMixer::new();
        let l = listener();
        // 90° to the side but within 320 units: center.
        m.request(
            9,
            Source::World {
                pos: (300, 0, 0),
                tag: 1,
            },
            &l,
        );
        assert_eq!(m.slots[9].pan, 0x7FFF);
        m.slots[9] = Slot::default();
        // Far to the side: panned off center.
        m.request(
            9,
            Source::World {
                pos: (5000, 0, 0),
                tag: 1,
            },
            &l,
        );
        assert_ne!(m.slots[9].pan, 0x7FFF);
    }

    #[test]
    fn keep_running_policy_does_not_restart() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = sounds();
        let mut m = FaithfulMixer::new();
        let l = listener();
        // id 37 (KRAKEN) = mode 3.
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                tag: 5,
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        let first: Vec<_> = rx.try_iter().collect();
        assert!(first.iter().any(|c| matches!(c, Cmd::Play { .. })));
        // Same (tag, id) again while "running" (mixer bookkeeping
        // says live because the mask still reports it).
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                tag: 5,
            },
            &l,
        );
        m.tick(&s, &tx, 1); // pretend ch0 still live
        let second: Vec<_> = rx.try_iter().collect();
        assert!(
            !second.iter().any(|c| matches!(c, Cmd::Play { .. })),
            "mode-3 sound restarted"
        );
    }

    #[test]
    fn restart_policy_restarts() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = sounds();
        let mut m = FaithfulMixer::new();
        let l = listener();
        m.request(
            9,
            Source::World {
                pos: (100, 0, 0),
                tag: 5,
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        rx.try_iter().count();
        m.request(
            9,
            Source::World {
                pos: (100, 0, 0),
                tag: 5,
            },
            &l,
        );
        m.tick(&s, &tx, 1);
        let cmds: Vec<_> = rx.try_iter().collect();
        assert!(cmds.iter().any(|c| matches!(c, Cmd::Stop { .. })));
        assert!(cmds.iter().any(|c| matches!(c, Cmd::Play { .. })));
    }

    #[test]
    fn ambient_loops_fade_in_and_switch() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = sounds();
        let mut m = FaithfulMixer::new();
        m.set_ambient(true, false, false); // over water
        m.tick(&s, &tx, 0);
        let cmds: Vec<_> = rx.try_iter().collect();
        // Waves loop started at vol 0 + one fade step.
        assert!(cmds.iter().any(|c| matches!(
            c,
            Cmd::Play {
                looped: true,
                vol: 0,
                ..
            }
        )));
        assert!(cmds.iter().any(|c| matches!(c, Cmd::SetVol { .. })));
        // Fade-in converges to 70<<8 - 1.
        for _ in 0..20 {
            m.tick(&s, &tx, m_live(&m));
        }
        let waves = m.find_channel(0, SND_WAVES).unwrap();
        assert_eq!(m.channels[waves].vol, (70 << 8) - 1);
        // Leaving water starts the wind loop and fades waves out.
        m.set_ambient(false, false, false);
        for _ in 0..30 {
            m.tick(&s, &tx, m_live(&m));
        }
        assert!(
            m.find_channel(0, SND_WAVES).is_none(),
            "waves not faded out"
        );
        assert!(m.find_channel(0, SND_WIND).is_some(), "wind not running");
    }

    /// Liveness mask synthesized from the mixer's own bookkeeping
    /// (tests have no real output stream).
    fn m_live(m: &FaithfulMixer) -> u32 {
        let mut mask = 0;
        for (i, c) in m.channels.iter().enumerate() {
            if c.key.is_some() {
                mask |= 1 << i;
            }
        }
        mask
    }
}
