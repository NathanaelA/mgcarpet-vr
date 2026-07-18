//! Faithful port of MC1's sound mixer (remc1, all citations
//! sub_main.cpp):
//!
//! - Request phase `sub_55370_558A0` (:64444): every sim sound event
//!   lands in ONE pending slot per sound id per tick, loudest request
//!   winning within an 8-unit tolerance (`sub_55870`: replace unless
//!   `new < old - 8`). Volume = linear falloff over a range that
//!   shrinks for sounds behind the listener — XY-ONLY distance
//!   (retail never reads z; Maths.cpp:738-742/1043-47), relative yaw
//!   folded to 0..1024 so the range runs 12288 ahead, 9216 at 90°,
//!   6144 directly behind (`12288·(1024 − rel/2) >> 10`); cull beyond
//!   12288² squared XY distance, drop below 512. Pan engages only
//!   beyond 320 units; the yaw re-folds to 0..512 (directly BEHIND =
//!   0 = center) with swing `folded << 6` — full deflection at 90°
//!   (both laws decompile-verified identical in remc1 AND remc2,
//!   Sound.cpp:6284-6329 / remc1:64474-64511; review 2026-07-15 D1
//!   fixed the port's z-inclusive distance, halved rear attenuation
//!   and half-swing pan).
//! - Flush phase `sub_55100_55630` (:64459): once per tick the 47
//!   slots issue channel operations. Mode 1 (`sub_483C0`) = restart:
//!   an already-running instance with the same (owner, id) is stopped
//!   first. Mode 3 (`sub_48470`) = don't-interrupt: the request is
//!   dropped while an (owner, id) instance still runs — the channel
//!   key is the PAIR (`word_12CD26` owner word + id; review
//!   2026-07-15 D2). 32 driver channels (`word_CBFF0`), free-channel
//!   allocation, no stealing (`sub_48570` silently drops when all 32
//!   are busy).
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

/// Request-slot table size. MC1's table is 47 entries (remc1
/// `sub_55100` loops `i < 47`); MC2's `EntitySounds_F4FE0` is 70
/// (Sound.h:56). One array at the larger size serves both — ids past a
/// game's own table fall to that game's `Policy::Drop` default, which
/// is exactly retail's bound behavior.
const SLOT_COUNT: usize = 70;
const CULL_DIST_SQ: i64 = 150_994_944; // 12288^2
const BASE_RANGE: i32 = 12288;
const MIN_VOL: i32 = 512;
const PAN_MIN_DIST: i32 = 320;
const FADE_STEP: i32 = 2048;
const FADE_FLOOR: i32 = 4096;
/// MC2's driver channel budget (`MaxSoundBufferChannels_E3794 = 10`,
/// Sound.cpp:14 — "Original was 10"); MC1 uses the full 32. No
/// stealing in either game — a full house drops the request.
const MC2_CHANNELS: usize = 10;
/// MC2 ids the dispatch pre-switch collapses onto owner 0 — ONE
/// shared channel per id regardless of emitter owner (Sound.cpp:
/// 6349-67: creature calls, gloops, door/tornado feeds).
const MC2_OWNER_COLLAPSE: [u8; 16] = [
    7, 32, 38, 42, 43, 44, 46, 47, 49, 50, 51, 52, 53, 58, 59, 62,
];

/// Per-id flush behavior (MC1: the `sub_55370` switch; MC2: the
/// `PrepareEventSound_6E450` dispatch).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Policy {
    /// Mode 1: restart a running (tag, id) instance.
    Restart,
    /// Mode 3: keep a running (tag, id) instance, drop the request.
    KeepRunning,
    /// Modes 1/3 but only accepted for player-sourced (or untagged)
    /// requests (MC1 ids 4, 14, 29 / 17; MC2 ids 14, 29 — the
    /// level-index gate: only the LOCAL player's Select/CantUse
    /// plays, Sound.cpp:6454-77).
    RestartPlayerOnly,
    KeepRunningPlayerOnly,
    /// Ambient loop with a fixed fade-in target (ids 1, 2, 5, 31).
    Loop(u16),
    /// MC2 playType 4 (DoorC2 47 / Tornado 49, EF:44324-31): a
    /// LOOPING channel keyed (0, id) at center pan, FED by the
    /// emitter every tick — volume rides each request; a starved
    /// tick fades it out (retail's emitters call EndLoop_6EAB0 on
    /// death = fade at ~30 Hz; a missing feed is the same signal,
    /// and the sim must not grow new stop events — they hash).
    /// ADJUDICATED against the loop-count ambiguity: the loop +
    /// per-request volume reading is the only one where retail's
    /// stop machinery (512 sentinel + EndLoop fade) is coherent
    /// (remc1's identical dead-code arm loops -1 and re-sets volume
    /// per flush).
    Feed,
    /// Everything else: the original's default case drops it.
    Drop,
}

fn policy_mc1(id: u8) -> Policy {
    match id {
        1 | 2 => Policy::Loop(70),
        5 => Policy::Loop(120),
        31 => Policy::Loop(85),
        3 | 9 | 15 | 16 | 18..=28 | 30 | 40 | 42..=45 => Policy::Restart,
        4 | 14 | 29 => Policy::RestartPlayerOnly,
        7 | 8 | 10..=13 | 32..=39 | 41 => Policy::KeepRunning,
        17 => Policy::KeepRunningPlayerOnly,
        _ => Policy::Drop,
    }
}

/// MC2's per-id law (`PrepareEventSound_6E450`, Sound.cpp:6347-6536,
/// over `EntitySounds_F4FE0[70]`): playType 1 → restart, playType 3 →
/// keep-running; Ocean/Crickets/Fire/Market are the ambient loops with
/// the same fade targets as MC1's. Ids the dispatch doesn't name fall
/// to its `default: return` — dropped. Previously every id ≥ 47 was
/// silently dropped and ids < 47 ran MC1's switch (review 2026-07-15
/// P0-5); still banked for D1: the playType-4 flush law (DoorC2 47
/// / Tornado 49 — keep-running stand-in) and the Select/CantUse/Hit
/// level-index gating.
fn policy_mc2(id: u8) -> Policy {
    match id {
        1 | 2 => Policy::Loop(70),
        5 => Policy::Loop(120),
        31 => Policy::Loop(85),
        // The level-index gate (Sound.cpp:6454-77): Select/CantUse
        // play only for the LOCAL player (flags forced 0 there);
        // world-sourced requests are dropped. (MC2, unlike MC1, does
        // NOT gate 4/17; the Hit set 54-57 passes -1 at its world
        // sites, so it stays plain playType 3.)
        14 | 29 => Policy::RestartPlayerOnly,
        // playType 1: spell/impact one-shots.
        3
        | 4
        | 6
        | 9..=11
        | 15
        | 18..=28
        | 30
        | 38
        | 40
        | 41
        | 48
        | 50..=53
        | 60
        | 61
        | 63
        | 64 => Policy::Restart,
        // playType 4: the emitter-fed loops (DoorC2 / Tornado).
        47 | 49 => Policy::Feed,
        // playType 3: creature calls (7..62), hits (54-57), cave
        // drips (65-69).
        7 | 8 | 12 | 13 | 16 | 17 | 32..=34 | 37 | 39 | 42..=44 | 46 | 54..=59 | 62 | 65..=69 => {
            Policy::KeepRunning
        }
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
    /// World-positioned: the emitter's coordinates drive the spatial
    /// volume/pan. Requests are keyed by SOUND ID — retail's
    /// `sub_55370_558A0` writes every request into the per-id slot
    /// `word_12CD24[5*id]`, so many entities playing the same sound at
    /// once (a meteor's trail of ground fires all crackling sound 3)
    /// contend for ONE request slot instead of flooding all 32
    /// channels. The CHANNEL key is the pair (`owner`, id) —
    /// `word_12CD26` is the emitter's OWNER word and `sub_483C0`
    /// matches both words — so a different owner's same-id sound gets
    /// its own channel while one owner's many emitters (the meteor
    /// case) still group (review 2026-07-15 D2; the sim resolves the
    /// emitter index to its owner tag at `take_audio` drain time).
    World { pos: (u16, u16, i16), owner: u16 },
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
    /// (owner, id) occupying this channel; None = believed free.
    /// Owner 0 = the player/ambient paths.
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
    /// Which game's per-id law applies (`policy_mc1` / `policy_mc2`).
    mc2: bool,
    /// The MC2 pitch-jitter LCG (ids 42-44/46) — audio-side only,
    /// never part of the sim hash.
    jitter: u32,
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
            mc2: false,
            jitter: 1,
        }
    }

    /// Switch to MC2's per-id sound law (`PrepareEventSound_6E450`).
    pub fn set_mc2(&mut self, on: bool) {
        self.mc2 = on;
    }

    /// Hard reset: stop every channel and forget all requests, fades
    /// and ambient wishes. The level-boundary teardown (retail stops
    /// the whole SFX system before a frontend transition, remc1
    /// `sub_5D010_5D520`): ambient loops and lingering one-shots die
    /// here instead of surviving under the menus.
    pub fn reset(&mut self, tx: &Sender<Cmd>) {
        for (i, ch) in self.channels.iter_mut().enumerate() {
            if ch.key.is_some() {
                let _ = tx.send(Cmd::Stop { ch: i });
            }
            *ch = Channel {
                key: None,
                looped: false,
                vol: 0,
                fade: Fade::None,
            };
        }
        self.slots = [Slot::default(); SLOT_COUNT];
        self.ambient = [false; 4];
    }

    /// The request phase: one sim sound event.
    pub fn request(&mut self, id: u8, source: Source, listener: &Listener) {
        if id as usize >= SLOT_COUNT {
            return;
        }
        let (vol, pan, tag, player_sourced) = match source {
            Source::Player => (0x7FFF_u16, 0x7FFF_u16, 0u16, true),
            Source::World { pos, owner } => {
                // Torus-wrapped deltas (the original's i16 truncation).
                // XY ONLY — retail's cull, falloff and pan threshold
                // never read z (Maths.cpp:738-742, 1043-47).
                let dx = i64::from(pos.0.wrapping_sub(listener.pos.0) as i16);
                let dy = i64::from(pos.1.wrapping_sub(listener.pos.1) as i16);
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > CULL_DIST_SQ {
                    return;
                }
                let dist = (dist_sq as f64).sqrt() as i32;
                // Horizontal bearing, engine 2048-space, 0 = north
                // (-y): forward = (sin yaw, -cos yaw).
                let bearing = ((dx as f64).atan2(-dy as f64) / std::f64::consts::TAU * 2048.0)
                    .rem_euclid(2048.0) as i32;
                // Relative yaw folded to 0..1024 (sub_582B0); the
                // side sign is taken BEFORE folding (sub_582F0).
                let off2048 = (bearing - i32::from(listener.yaw)).rem_euclid(2048);
                let right = off2048 > 0 && off2048 < 1024;
                let ahead_or_behind = off2048 == 0 || off2048 == 1024;
                let rel = if off2048 > 1024 {
                    2048 - off2048
                } else {
                    off2048
                };
                // Audible range: 12288 ahead → 9216 at 90° → 6144
                // directly behind (`12288·(1024 − rel/2) >> 10`).
                let range = (BASE_RANGE * (1024 - rel / 2)) >> 10;
                let vol = 0x7FFF * (range - dist) / range.max(1);
                if vol < MIN_VOL {
                    return;
                }
                let vol = vol.min(0x7FFF) as u16;
                let pan = if dist > PAN_MIN_DIST && !ahead_or_behind {
                    // Re-fold to 0..512: directly behind = 0 = CENTER;
                    // swing `folded << 6` = full deflection at 90°.
                    let folded = if rel > 512 { 1024 - rel } else { rel };
                    let swing = folded << 6;
                    let p = 0x7FFF + if right { swing } else { -swing };
                    p.clamp(0, 0xFFFF) as u16
                } else {
                    0x7FFF
                };
                // Channel identity = (owner, id) — see `Source`; the
                // MC2 dispatch collapses a fixed id set onto owner 0
                // (the Sound.cpp:6349-67 pre-switch).
                let owner = if self.mc2 && MC2_OWNER_COLLAPSE.contains(&id) {
                    0
                } else {
                    owner
                };
                (vol, pan, owner, false)
            }
        };

        let policy = if self.mc2 { policy_mc2 } else { policy_mc1 };
        match policy(id) {
            Policy::Drop => {}
            Policy::Loop(_) => {} // loops are driven by ambient state
            p => {
                let slot = &mut self.slots[id as usize];
                // sub_55870: replace unless clearly quieter. MC2's
                // cave-drip arm (65-69) writes UNCONDITIONALLY — no
                // -8 test, last request wins (Sound.cpp:6523-34).
                let drip = self.mc2 && (65..=69).contains(&id);
                if slot.pending && !drip && i32::from(vol) < i32::from(slot.vol) - 8 {
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

        // Slot flush (sub_55100 / PlayEntitySounds_6E150).
        let policy = if self.mc2 { policy_mc2 } else { policy_mc1 };
        let mut fed = [false; SLOT_COUNT];
        for id in 0..SLOT_COUNT as u8 {
            let slot = self.slots[id as usize];
            if !slot.pending {
                continue;
            }
            self.slots[id as usize] = Slot::default();
            // playType 4 (EF:44324-31): the emitter-fed loop — one
            // shared (0, id) channel, CENTER pan, volume riding each
            // feed; started looped when absent.
            if policy(id) == Policy::Feed {
                fed[id as usize] = true;
                if let Some(i) = self.find_channel(0, id) {
                    self.channels[i].vol = i32::from(slot.vol);
                    self.channels[i].fade = Fade::None;
                    let _ = tx.send(Cmd::SetVol {
                        ch: i,
                        vol: slot.vol,
                    });
                } else if let Some(free) = self.free_channel(live_mask) {
                    if let Some(pcm) = sample(sounds, id) {
                        let _ = tx.send(Cmd::Play {
                            ch: free,
                            pcm,
                            sample_rate: sounds.sample_rate,
                            vol: slot.vol,
                            pan: 0x7FFF,
                            looped: true,
                        });
                        self.channels[free] = Channel {
                            key: Some((0, id)),
                            looped: true,
                            vol: i32::from(slot.vol),
                            fade: Fade::None,
                        };
                    }
                }
                continue;
            }
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
                continue; // all channels busy: the original drops too
            };
            let Some(pcm) = sample(sounds, id) else {
                continue;
            };
            // MC2 pitch jitter (Sound.cpp:6331-45): devil calls 42-44
            // roll ±15% and the gloop 46 ±10% per play (the emitter-
            // action +10..+30 variant of 46 is unmodeled — APPROX;
            // the mixer's own LCG, audio is outside the sim hash).
            let mut rate = sounds.sample_rate;
            if self.mc2 && matches!(id, 42..=44 | 46) {
                self.jitter = self.jitter.wrapping_mul(9377).wrapping_add(9439);
                let j = if id == 46 {
                    (self.jitter % 20) as i32 - 10
                } else {
                    (self.jitter % 30) as i32 - 15
                };
                rate = (i64::from(rate) * i64::from(100 + j) / 100).max(1) as u32;
            }
            let _ = tx.send(Cmd::Play {
                ch: free,
                pcm,
                sample_rate: rate,
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
        // Feed starvation: a fed loop with no request this tick lost
        // its emitter (death/despawn) — fade it out, retail's
        // EndLoop_6EAB0 net effect (the sim must not grow new stop
        // events; they hash).
        if self.mc2 {
            for ch in self.channels.iter_mut() {
                if let Some((0, id)) = ch.key {
                    if policy(id) == Policy::Feed
                        && !fed[id as usize]
                        && ch.fade != Fade::Out
                        && ch.looped
                    {
                        ch.fade = Fade::Out;
                    }
                }
            }
        }
    }

    fn find_channel(&self, tag: u16, id: u8) -> Option<usize> {
        self.channels.iter().position(|c| c.key == Some((tag, id)))
    }

    fn free_channel(&self, live_mask: u32) -> Option<usize> {
        // MC2 runs 10 driver channels, MC1 all 32; neither steals.
        let cap = if self.mc2 { MC2_CHANNELS } else { CHANNELS };
        self.channels
            .iter()
            .take(cap)
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
                owner: 1,
            },
            &listener(),
        );
        assert!(!m.slots[9].pending);
        // Close: loud.
        m.request(
            9,
            Source::World {
                pos: (1000, 0, 0),
                owner: 1,
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
                owner: 1,
            },
            &l,
        );
        let quiet = m.slots[9].vol;
        m.request(
            9,
            Source::World {
                pos: (500, 0, 0),
                owner: 1,
            },
            &l,
        );
        let loud = m.slots[9].vol;
        assert!(loud > quiet);
        // A far request must not displace the near one.
        m.request(
            9,
            Source::World {
                pos: (9000, 0, 0),
                owner: 1,
            },
            &l,
        );
        assert_eq!(m.slots[9].vol, loud);
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
                owner: 1,
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
                owner: 1,
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
                owner: 1,
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        let first: Vec<_> = rx.try_iter().collect();
        assert!(first.iter().any(|c| matches!(c, Cmd::Play { .. })));
        // Same id again while "running" (mixer bookkeeping says live
        // because the mask still reports it).
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                owner: 1,
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

    /// The channel key is the (OWNER, id) PAIR (review 2026-07-15
    /// D2): a different owner's same-id sound gets its OWN channel
    /// (rival casts no longer suppress/restart player-owned sounds),
    /// while one owner's repeats still hit the keep-running/restart
    /// law on its own channel.
    #[test]
    fn channel_key_is_the_owner_id_pair() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = sounds();
        let mut m = FaithfulMixer::new();
        let l = listener();
        // id 37 = mode 3 (keep-running). Owner 1 starts it.
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                owner: 1,
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        assert_eq!(
            rx.try_iter()
                .filter(|c| matches!(c, Cmd::Play { .. }))
                .count(),
            1
        );
        // Owner 2, same id, while owner 1 still runs: a SECOND
        // channel plays (the old constant-0 tag dropped this).
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                owner: 2,
            },
            &l,
        );
        m.tick(&s, &tx, m_live(&m));
        assert_eq!(
            rx.try_iter()
                .filter(|c| matches!(c, Cmd::Play { .. }))
                .count(),
            1,
            "a different owner's same-id sound gets its own channel"
        );
        assert!(m.find_channel(1, 37).is_some());
        assert!(m.find_channel(2, 37).is_some());
        // Owner 1 again: keep-running on ITS channel — no new play.
        m.request(
            37,
            Source::World {
                pos: (100, 0, 0),
                owner: 1,
            },
            &l,
        );
        m.tick(&s, &tx, m_live(&m));
        assert_eq!(
            rx.try_iter()
                .filter(|c| matches!(c, Cmd::Play { .. }))
                .count(),
            0,
            "the same owner's repeat keeps running (mode 3)"
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
                owner: 1,
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        rx.try_iter().count();
        m.request(
            9,
            Source::World {
                pos: (100, 0, 0),
                owner: 1,
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

    /// MC2 playType 4 (47/49): the emitter-fed loop — starts looped
    /// at CENTER pan on the shared owner-0 channel, volume rides
    /// each feed without a restart, and a starved tick (dead
    /// emitter) fades it out — the EndLoop_6EAB0 net effect (review
    /// 2026-07-15 D1).
    #[test]
    fn mc2_feed_loop_follows_the_emitter() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = sounds();
        let mut m = FaithfulMixer::new();
        m.set_mc2(true);
        let l = listener();
        m.request(
            49,
            Source::World {
                pos: (1000, 0, 0),
                owner: 7, // collapse set → keyed (0, 49)
            },
            &l,
        );
        m.tick(&s, &tx, 0);
        let cmds: Vec<_> = rx.try_iter().collect();
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                Cmd::Play {
                    looped: true,
                    pan: 0x7FFF,
                    ..
                }
            )),
            "the tornado feed starts as a center-pan loop"
        );
        // Fed again nearer: volume updates, NO restart.
        m.request(
            49,
            Source::World {
                pos: (200, 0, 0),
                owner: 7,
            },
            &l,
        );
        m.tick(&s, &tx, m_live(&m));
        let cmds: Vec<_> = rx.try_iter().collect();
        assert!(!cmds.iter().any(|c| matches!(c, Cmd::Play { .. })));
        assert!(cmds.iter().any(|c| matches!(c, Cmd::SetVol { .. })));
        // Starved (emitter died): fades out and clears.
        for _ in 0..30 {
            m.tick(&s, &tx, m_live(&m));
        }
        assert!(
            m.find_channel(0, 49).is_none(),
            "a starved feed loop fades out"
        );
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
