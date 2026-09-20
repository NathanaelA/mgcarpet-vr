//! In-app `.mgcr` playback (`--replay`) and the port recorder
//! (`--record`) — docs/RECORDING.md "Consumers".
//!
//! `--replay` is SOURCE-AGNOSTIC (player-ruled): a retail take plays
//! through inline input recovery (the shared laws in
//! `mgc_formats::recover`, the seeding in mgc-sim's conformance
//! module — one implementation with `mgc-conform replay`), a port
//! take (`source:"port"`, `channels.input:"exact"`) feeds its
//! recorded `FlightInput` stream directly. Both run PURE: the world
//! free-runs on the recorded input, divergence is REPORTED (the HUD
//! counter, the console), never corrected. A gap in a retail take
//! re-anchors a fresh segment from the next closure — a capture
//! artifact, not a resync.
//!
//! The tick cadence maps one recorded pair to one sim tick, so the
//! app's own pacing machinery (F3 game speed, P pause) is the
//! playback transport for free.

use crate::LoadedLevel;
use mgc_formats::mgcr::{
    self, Family, ObsMc1, PortInput, Recording, RecordingWriter, RetailMc1, RetailMc2,
    TerrainImage, decode_retail_mc1, decode_retail_mc2,
};
use mgc_formats::recover;
use mgc_sim::engine::features::Planes;
use mgc_sim::engine::world::ImportPin;
use mgc_sim::engine::world::conformance::{self, ThingTable};
use mgc_sim::flight::Mc2Ext;
use mgc_sim::mc1::spells::SpellId;
use mgc_sim::{AltitudeModel, FlightInput, Simulation, ThrustModel};
use serde::Deserialize as _;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaySource {
    Retail,
    Port,
}

/// The opened take, header decoded — `game_main` resolves the level
/// path from it, then hands it to the App to become a driver once the
/// session exists.
pub struct ReplayFile {
    rec: Recording,
    pub game: String,
    pub level: u32,
    pub family: Family,
    pub source: ReplaySource,
    /// Port header sim closure: the recorded tier tags ("classic" /
    /// "enhanced") and the embedded start snapshot.
    pub sim_thrust: Option<String>,
    pub sim_altitude: Option<String>,
    /// The recorded retail-bug patch policy tag (see
    /// [`ReplayFile::patch_policy`]); absent on pre-option takes.
    pub sim_patches: Option<String>,
    /// The OFFLINE chassis overrides the take was recorded with
    /// (`None` = the faithful default, and also what a pre-key take
    /// reads as). The replay must build its world with these BEFORE
    /// applying `snapshot` — they are what `snap_check_identity`
    /// refuses on.
    pub sim_pool_slots: Option<usize>,
    pub sim_awake_range: Option<u32>,
    /// MC2 retail takes: the campaign REPLAY gate, derived from the
    /// capture's own witness exactly as the conformance runner derives
    /// it ([`mgcr::mc2_take_replayed`]). Scanned once at open — it is a
    /// run-constant — and applied to the world at every anchor.
    pub mc2_replayed: bool,
    /// The import pin the take was recorded under
    /// ([`mgc_sim::engine::world::World::import_pin`]) — the world
    /// state a snapshot deliberately does not carry (the MC1 acq
    /// list, the hand bits). `None` = the header has no key: either
    /// the pin was all-default at record time, or the take predates
    /// live `--record` writing its real pin. Both replay by LEAVING
    /// the natively-built world's pin alone — clobbering it to
    /// all-default is what emptied a plausible-spellbook take's
    /// spellbook (the acq list survives only in the pin, and zeroing
    /// it desyncs the take at t=1).
    pub sim_import_pin: Option<ImportPin>,
    pub snapshot: Option<Vec<u8>>,
}

impl ReplayFile {
    pub fn open(path: &Path) -> Result<ReplayFile, String> {
        let rec = Recording::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let family = rec.header.family()?;
        let level = rec.header.level.ok_or("recording has no level number")?;
        let source = match rec.header.source.as_str() {
            "retail" => {
                if !rec.header.channels.state {
                    return Err(
                        "retail take has no state channel — input cannot be recovered".into(),
                    );
                }
                ReplaySource::Retail
            }
            "port" => {
                if rec.header.channels.input != "exact" {
                    return Err(format!(
                        "port recording has input channel {:?}, need \"exact\"",
                        rec.header.channels.input
                    ));
                }
                ReplaySource::Port
            }
            other => return Err(format!("unknown recording source {other:?}")),
        };
        let sim = rec.header.sim.as_ref();
        let s = |key: &str| {
            sim.and_then(|s| s.get(key))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        };
        if source == ReplaySource::Port {
            // The header's sim closure PINS the run — a snapshot from
            // another format version is a refusal, not a warning
            // (docs/RECORDING.md "Consumers").
            let ver = sim
                .and_then(|s| s.get("snapshot_version"))
                .and_then(|v| v.as_u64());
            if ver != Some(mgc_sim::snapshot::SNAPSHOT_VERSION as u64) {
                return Err(format!(
                    "port recording pins snapshot version {ver:?}, this build reads {}",
                    mgc_sim::snapshot::SNAPSHOT_VERSION
                ));
            }
        }
        let snapshot = sim
            .and_then(|s| s.get("start_mgcs_b64"))
            .and_then(|v| v.as_str())
            .map(mgcr::b64_decode)
            .transpose()?;
        let n = |key: &str| sim.and_then(|s| s.get(key)).and_then(|v| v.as_u64());
        // The REPLAY gate is a property of the CAPTURE, not of the
        // header, so it costs one scan to the first (14,5) scroll.
        let mc2_replayed = if family == Family::Mc2 && source == ReplaySource::Retail {
            mgcr::mc2_take_replayed(path)?
        } else {
            false
        };
        Ok(ReplayFile {
            game: rec.header.game.clone(),
            level,
            family,
            source,
            sim_thrust: s("thrust_model"),
            sim_altitude: s("altitude_model"),
            sim_patches: s("patches"),
            sim_pool_slots: n("entity_pool_size").map(|v| v as usize),
            sim_awake_range: n("awake_range").map(|v| v as u32),
            mc2_replayed,
            sim_import_pin: sim.and_then(|s| s.get("import_pin")).map(|p| ImportPin {
                strict_retail: p
                    .get("strict_retail")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                measured_terrain: p
                    .get("measured_terrain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                carpet_slot: p.get("carpet_slot").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
                castle_reg: pin_u16s(p, "castle_reg"),
                human_pose_prev: {
                    let a = pin_i64s::<3>(p, "human_pose_prev");
                    (a[0] as u16, a[1] as u16, a[2] as i16)
                },
                human_yaw: pin_u64(p, "human_yaw") as u16,
                human_yaw_prev: pin_u64(p, "human_yaw_prev") as u16,
                hand_bits: pin_u64(p, "mc1_hand_bits") as u32,
                mc1_cast_pose: {
                    let a = pin_i64s::<6>(p, "mc1_cast_pose");
                    mgc_sim::engine::world::PlayerPose {
                        x: a[0] as u16,
                        y: a[1] as u16,
                        z: a[2] as i16,
                        heading: a[3] as u16,
                        pitch: a[4] as u16,
                        speed: a[5] as i16,
                    }
                },
                // i32 verbatim — the dead-window −1 sentinels are
                // real state (old u16-era pins re-parse fine: JSON
                // numbers are typeless and were all non-negative).
                mc1_acq: pin_i64s::<{ mgc_sim::mc1::spells::SPELL_COUNT }>(p, "mc1_acq")
                    .map(|v| v as i32),
                mc2_turn: pin_u64(p, "mc2_turn") as u32,
                mc2_carpet_stall: pin_bool(p, "mc2_carpet_stall"),
                mc1_frame: pin_u64(p, "mc1_frame") as u32,
                // Older pins predate the key: the HUD was drawn.
                mc1_hud_drawn: p
                    .get("mc1_hud_drawn")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                mc1_v14: pin_bool(p, "mc1_v14"),
                accel_veto: {
                    let a = p.get("accel_veto").and_then(|v| v.as_array());
                    let at = |i: usize| {
                        a.and_then(|a| a.get(i))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    };
                    (at(0), at(1))
                },
                pending_teleport: pin_f32s::<3>(p, "pending_teleport")
                    .map(|a| (a[0], a[1], Some(a[2]))),
                pending_respawn: pin_f32s::<3>(p, "pending_respawn").map(|a| (a[0], a[1], a[2])),
                pending_restart: pin_bool(p, "pending_restart"),
                wiz_charge: {
                    let mut r = [0u8; 8];
                    if let Some(a) = p.get("wiz_charge").and_then(|v| v.as_array()) {
                        for (slot, v) in r.iter_mut().zip(a) {
                            *slot = v.as_u64().unwrap_or(0) as u8;
                        }
                    }
                    r
                },
            }),
            snapshot,
            rec,
        })
    }

    /// The recorded tier tags as sim enums (port takes; a missing
    /// closure defaults faithful).
    pub fn models(&self) -> Result<(ThrustModel, AltitudeModel), String> {
        let thrust = match self.sim_thrust.as_deref() {
            None | Some("classic") => ThrustModel::Mc1,
            Some("enhanced") => ThrustModel::Enhanced,
            Some(other) => return Err(format!("unknown thrust_model {other:?}")),
        };
        let altitude = match self.sim_altitude.as_deref() {
            None | Some("classic") => AltitudeModel::Faithful,
            Some("enhanced") => AltitudeModel::ExtendedLift,
            Some(other) => return Err(format!("unknown altitude_model {other:?}")),
        };
        Ok((thrust, altitude))
    }

    /// The patch policy this take was recorded under (port takes).
    /// `"retail"` (every take since --record forced the retail arms)
    /// -> replay under GameplayPatches::retail_all(); an ABSENT key is
    /// a take from before the patches were options -> the legacy
    /// hard-wired set. Retail-source takes always pin retail arms.
    pub fn patch_policy(&self) -> Result<crate::config::GameplayPatches, String> {
        if self.source == ReplaySource::Retail {
            return Ok(crate::config::GameplayPatches::retail_all());
        }
        match self.sim_patches.as_deref() {
            Some("retail") => Ok(crate::config::GameplayPatches::retail_all()),
            None => Ok(crate::config::GameplayPatches::legacy()),
            Some(other) => Err(format!("unknown patches policy {other:?}")),
        }
    }
}

/// Import-pin field readers. A MISSING key reads as the default, so
/// the pin can grow without invalidating takes written before the
/// field existed — they were recorded when it was not yet carried, and
/// nothing else can be said about them.
fn pin_u64(p: &serde_json::Value, key: &str) -> u64 {
    p.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn pin_bool(p: &serde_json::Value, key: &str) -> bool {
    p.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// `None` for an absent or null key — these lanes are `Option`s whose
/// emptiness is meaningful, not a missing-field default.
fn pin_f32s<const N: usize>(p: &serde_json::Value, key: &str) -> Option<[f32; N]> {
    let a = p.get(key)?.as_array()?;
    let mut out = [0f32; N];
    for (slot, v) in out.iter_mut().zip(a) {
        *slot = v.as_f64().unwrap_or(0.0) as f32;
    }
    Some(out)
}

fn pin_u16s<const N: usize>(p: &serde_json::Value, key: &str) -> [u16; N] {
    let mut out = [0u16; N];
    if let Some(a) = p.get(key).and_then(|v| v.as_array()) {
        for (slot, v) in out.iter_mut().zip(a) {
            *slot = v.as_u64().unwrap_or(0) as u16;
        }
    }
    out
}

fn pin_i64s<const N: usize>(p: &serde_json::Value, key: &str) -> [i64; N] {
    let mut out = [0i64; N];
    if let Some(a) = p.get(key).and_then(|v| v.as_array()) {
        for (slot, v) in out.iter_mut().zip(a) {
            *slot = v.as_i64().unwrap_or(0);
        }
    }
    out
}

enum RetailPrev {
    Mc1(Box<RetailMc1>),
    Mc2(Box<RetailMc2>),
}

/// The live playback driver: pulls records, recovers/decodes input,
/// performs retail anchors, grades divergence after each step.
pub struct ReplayDriver {
    rec: Recording,
    source: ReplaySource,
    family: Family,
    // Retail chain state.
    timg: Option<TerrainImage>,
    /// The take's campaign REPLAY gate ([`ReplayFile::mc2_replayed`]),
    /// re-applied to the world at every MC2 anchor.
    mc2_replayed: bool,
    pristine: Option<Planes>,
    things: Option<ThingTable>,
    witness: recover::Mc2RespawnWitness,
    prev: Option<(u64, RetailPrev)>,
    human_slot: u16,
    /// One pair was fed and awaits its post-step grade (`true` =
    /// capture-clean, gradeable).
    pending: Option<bool>,
    anchored_flag: bool,
    /// ⭐ THE PERMADEATH RESYNC (player-ruled 2026-09-07). The port
    /// signalled a level restart, so the next retail boundary
    /// re-anchors on the RECORDING instead of rebuilding a pristine
    /// world. Retail's reload runs machinery whose inputs are not in
    /// the capture — MC1 re-reads the level over its own live heap
    /// residue (`sub_408D0` :51596, RNG continuing MID-STREAM: there
    /// is no `srand` in the binary), MC2 restores a PRE-CAPTURE disk
    /// checkpoint (SaveLevel slot 1, EF:39894-921) — so no amount of
    /// native init can reproduce it. Re-importing retail's own
    /// closure sidesteps the whole question, which is what lets an
    /// MC2 take carry a permadeath at all.
    force_anchor: bool,
    // Tallies.
    segments: u64,
    /// Anchors that were FORCED by a restart rather than opened by a
    /// capture gap — reported separately so a resync can never be
    /// mistaken for a clean run.
    resyncs: u64,
    steps: u64,
    graded: u64,
    /// Clean boundaries BEFORE the first divergence — i.e. the horizon,
    /// which is what `mgc-conform replay --brief`'s `clean=` means. The
    /// two instruments' headline numbers are only comparable if this
    /// one stops where that one stops.
    clean: u64,
    /// Clean boundaries after it (a transient divergence recovers;
    /// a permanent one never does).
    clean_after: u64,
    skipped: u64,
    stick_unrec: u64,
    /// Boundaries retail spent PAUSED (`sub_41780`'s `+2 & 1` arm,
    /// :52224-25) — one LCG draw, no entity walk, so this driver
    /// consumes the record without a `sim.step`.
    paused: u64,
    diverged: Option<(u64, String)>,
    /// The on-screen counter line (④): refreshed every boundary.
    pub hud: String,
    /// The recorded pose for the GHOST billboard: (x, alt, z) tiles,
    /// yaw radians, sprite type index.
    pub ghost: Option<(f32, f32, f32, f32, u16)>,
    /// The current row's live-option events (port takes), already
    /// applied to the sim by `next_port` — a re-recording drains them
    /// into its own rows via [`ReplayDriver::take_row_set`].
    row_set: serde_json::Map<String, serde_json::Value>,
    pub finished: bool,
}

const GHOST_RAD: f32 = std::f32::consts::TAU / 2048.0;

impl ReplayDriver {
    /// Turn an opened take into the live driver, capturing the
    /// world-side reset state (the session must be installed; for
    /// port takes the snapshot must already be restored).
    pub fn install(file: ReplayFile, sim: &mut Simulation) -> Result<ReplayDriver, String> {
        let (pristine, things) = match file.source {
            ReplaySource::Retail => {
                let w = sim
                    .world
                    .as_ref()
                    .ok_or("replay needs a living world (the level did not build one)")?;
                let things = matches!(file.family, Family::Mc2).then(|| w.thing_table_clone());
                (Some(w.planes_clone()), things)
            }
            ReplaySource::Port => (None, None),
        };
        let timg = file
            .rec
            .header
            .channels
            .terrain
            .as_ref()
            .map(TerrainImage::new);
        Ok(ReplayDriver {
            rec: file.rec,
            source: file.source,
            family: file.family,
            timg,
            mc2_replayed: file.mc2_replayed,
            pristine,
            things,
            witness: recover::Mc2RespawnWitness::default(),
            prev: None,
            human_slot: 0,
            pending: None,
            anchored_flag: false,
            force_anchor: false,
            segments: 0,
            resyncs: 0,
            steps: 0,
            graded: 0,
            clean: 0,
            clean_after: 0,
            skipped: 0,
            stick_unrec: 0,
            paused: 0,
            diverged: None,
            hud: String::from("REPLAY starting"),
            ghost: None,
            row_set: serde_json::Map::new(),
            finished: false,
        })
    }

    /// Drain the current row's applied live-option events (see
    /// [`ReplayDriver::row_set`]).
    pub fn take_row_set(&mut self) -> serde_json::Map<String, serde_json::Value> {
        std::mem::take(&mut self.row_set)
    }

    /// The next tick's input, driving anchors/segments along the way.
    /// `None` = the take ended (or a fatal record error) — the caller
    /// hands control back to the player.
    pub fn next(&mut self, sim: &mut Simulation) -> Option<FlightInput> {
        // ⭐⭐ THE PERMADEATH POLL LIVES HERE, AND THAT PLACEMENT IS
        // THE FIX TO A SPEED-DEPENDENT BUG. `next` is called exactly
        // once per sim step, so polling here is per STEP for every
        // caller for free. The app's interactive loop runs N steps
        // per rendered FRAME at game speed N, and it used to poll
        // after the whole burst: at 1x that was the very next
        // boundary and the resync landed in time, but above 1x the
        // rest of the burst kept feeding recorded input into the
        // UN-RESTARTED world, and `grade` latches the first
        // divergence FOREVER — so mc1l48 read "DIVERGED since
        // t=8592" (the reload seam) sped up and ran clean through it
        // at normal speed. Player-witnessed. The headless
        // `replay_check` loop happened to poll per step, so it never
        // showed the bug, which is exactly why the poll must not
        // live in either loop.
        //
        // Retail only: a PORT take replays its own restart, so the
        // app must still route it to `restart_level`.
        if matches!(self.source, ReplaySource::Retail)
            && sim.world.as_mut().is_some_and(|w| w.take_restart())
        {
            self.force_anchor = true;
        }
        let input = match self.source {
            ReplaySource::Port => self.next_port(sim),
            ReplaySource::Retail => self.next_retail(sim),
        };
        // ⭐ STAMP THE TAKE'S TICK FOR THE WORLD-SIDE PROBES. Every
        // `MGC_*_TRACE` in the sim labels itself from
        // [`mgc_sim::DEBUG_TICK`], and only the conform drivers had ever
        // stamped it — so the app, the second retail instrument, ran the
        // whole shared trace suite reading `t=0`. One line buys
        // `MGC_CARPET_PROBE`, `MGC_WRITE_TRACE`, `MGC_MAIL_TRACE` and
        // the rest on this driver, with the same tick numbering conform
        // uses (the tick the upcoming step PRODUCES, which is the tick
        // `grade` then reports).
        if let Some((t, _)) = &self.prev {
            mgc_sim::DEBUG_TICK.store(*t, std::sync::atomic::Ordering::Relaxed);
        }
        input
    }

    /// The world was re-imported since the last call (the caller
    /// clears its render mirrors — stale (slot, generation) pose
    /// pairs must not survive an import).
    pub fn take_anchored(&mut self) -> bool {
        std::mem::take(&mut self.anchored_flag)
    }

    /// Arm the permadeath resync: the next retail boundary re-anchors
    /// on the recording (see [`ReplayDriver::force_anchor`]). The app
    /// calls this INSTEAD of `restart_level` while a retail take is
    /// driving — a pristine rebuild would replace retail's world with
    /// the port's own native level init, which is a different world
    /// (mc1l48 at its own phase: port 942 live / 22 families against
    /// retail's 943 / 40) and detonates the run on the spot.
    ///
    /// A port take needs none of this: it re-runs the port's own
    /// restart, which is exactly what it recorded.
    pub fn arm_resync(&mut self) {
        if matches!(self.source, ReplaySource::Retail) {
            self.force_anchor = true;
        }
    }

    /// Is a retail take driving? (`--replay` of a port take restarts
    /// the level the ordinary way.)
    pub fn is_retail(&self) -> bool {
        matches!(self.source, ReplaySource::Retail)
    }

    fn finish_stream(&mut self) {
        self.finished = true;
        println!("replay: {}", self.summary());
    }

    pub fn summary(&self) -> String {
        // Paused boundaries are reported SEPARATELY from capture
        // skips: retail ran them, they just ran nothing (`sub_41780`
        // :52224-25). Folding them into "capture-skipped" reads as
        // recorder loss, which is the opposite of what they are.
        let paused = if self.paused > 0 {
            format!(", {} paused", self.paused)
        } else {
            String::new()
        };
        // A permadeath resync is a re-import of retail's closure, so
        // it MUST be visible: the clean count downstream of one was
        // not earned by free-running from the seed.
        let resyncs = if self.resyncs > 0 {
            format!(", {} permadeath resync(es)", self.resyncs)
        } else {
            String::new()
        };
        let base = format!(
            "{} tick(s) in {} segment(s), {} graded ({} capture-skipped{paused}), {} clean{resyncs}",
            self.steps, self.segments, self.graded, self.skipped, self.clean
        );
        match &self.diverged {
            Some((t, lane)) => format!(
                "{base} (+{} clean after); DIVERGED since t={t} ({lane})",
                self.clean_after
            ),
            None => format!("{base}; bit-exact throughout"),
        }
    }

    fn refresh_hud(&mut self, t: u64) {
        self.hud = match (&self.diverged, self.source) {
            (Some((dt, lane)), _) => format!("REPLAY t={t} - diverged since t={dt} ({lane})"),
            (None, ReplaySource::Retail) => {
                format!("REPLAY t={t} - pose bit-exact ({} graded)", self.graded)
            }
            (None, ReplaySource::Port) => {
                format!("REPLAY t={t} - hash-exact ({} graded)", self.graded)
            }
        };
    }

    // ---------------------------------------------------------- port

    fn next_port(&mut self, sim: &mut Simulation) -> Option<FlightInput> {
        let tick = match self.rec.next_tick() {
            None => {
                self.finish_stream();
                return None;
            }
            Some(Err(e)) => {
                eprintln!("replay: record error: {e}");
                self.finish_stream();
                return None;
            }
            Some(Ok(t)) => t,
        };
        // Live-option events recorded on this row: the player applied
        // them from the running app between the previous row and this
        // row's hash point, so they replay HERE — before the grade —
        // through the same setters the options menu uses. An
        // unimplemented key is a refusal, not a shrug: the take's
        // whole course depends on it.
        self.row_set.clear();
        if let Some(set) = &tick.set {
            if let Err(e) = apply_live_options(sim, set) {
                eprintln!("replay: t={}: {e}", tick.t);
                self.finish_stream();
                return None;
            }
            self.row_set = set.clone();
        }
        // The hash channel describes tick t's PRE-input state — the
        // sim sits exactly there right now (the phase convention).
        if let Some(want) = tick.hash {
            self.graded += 1;
            let got = sim.state_hash();
            if want == got {
                // Clean = the horizon, same rule as the retail arm's
                // grade.
                if self.diverged.is_none() {
                    self.clean += 1;
                } else {
                    self.clean_after += 1;
                }
            } else if self.diverged.is_none() {
                self.diverged = Some((tick.t, "hash".into()));
                println!(
                    "replay: hash desync at t={} — recorded {want:016x}, live {got:016x}",
                    tick.t
                );
            }
        }
        let input = match &tick.input {
            Some(v) => match PortInput::deserialize(v) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("replay: t={}: bad input record: {e}", tick.t);
                    self.finish_stream();
                    return None;
                }
            },
            None => PortInput::default(),
        };
        self.steps += 1;
        if self.segments == 0 {
            self.segments = 1;
        }
        // The port arm keeps no `prev`, so it stamps its own tick (see
        // [`ReplayDriver::next`]).
        mgc_sim::DEBUG_TICK.store(tick.t, std::sync::atomic::Ordering::Relaxed);
        self.refresh_hud(tick.t);
        Some(flight_input_from(&input))
    }

    // -------------------------------------------------------- retail

    fn next_retail(&mut self, sim: &mut Simulation) -> Option<FlightInput> {
        loop {
            let tick = match self.rec.next_tick() {
                None => {
                    self.finish_stream();
                    return None;
                }
                Some(Err(e)) => {
                    eprintln!("replay: record error: {e}");
                    self.finish_stream();
                    return None;
                }
                Some(Ok(t)) => t,
            };
            // The terrain image tracks the take continuously
            // (self-healing deltas) — installed only at anchors.
            if let (Some(img), Some(block)) = (self.timg.as_mut(), &tick.terrain) {
                if let Err(e) = img.apply(block) {
                    eprintln!("replay: t={}: terrain: {e}", tick.t);
                }
            }
            let Some(state) = &tick.state else {
                self.prev = None;
                continue;
            };
            let out = match self.family {
                Family::Mc1 => match decode_retail_mc1(state) {
                    Ok(st) => self.retail_tick_mc1(sim, &tick, st),
                    Err(e) => {
                        eprintln!("replay: t={}: {e}", tick.t);
                        self.finish_stream();
                        return None;
                    }
                },
                Family::Mc2 => match decode_retail_mc2(state) {
                    Ok(st) => self.retail_tick_mc2(sim, &tick, st),
                    Err(e) => {
                        eprintln!("replay: t={}: {e}", tick.t);
                        self.finish_stream();
                        return None;
                    }
                },
            };
            match out {
                Ok(Some(input)) => return Some(input),
                Ok(None) => continue, // anchored — pull the next record
                Err(e) => {
                    eprintln!("replay: t={}: {e}", tick.t);
                    self.finish_stream();
                    return None;
                }
            }
        }
    }

    fn retail_tick_mc1(
        &mut self,
        sim: &mut Simulation,
        tick: &mgcr::TickRecord,
        st: RetailMc1,
    ) -> Result<Option<FlightInput>, String> {
        let forced = std::mem::take(&mut self.force_anchor);
        let anchor =
            forced || !matches!(&self.prev, Some((pt, RetailPrev::Mc1(_))) if tick.t == pt + 1);
        if anchor {
            let w = sim.world.as_mut().ok_or("no world")?;
            if forced {
                // Retail's reload RE-READ the level: un-consume every
                // one-shot disposition the previous life fired. The
                // entity import restores the pool, but the THING
                // table is port-side state (the conform twin does the
                // same at its restart re-anchor).
                w.reload_thing_table();
                self.resyncs += 1;
            }
            w.restore_planes(self.pristine.as_ref().expect("retail install"));
            let report = w
                .retail_import_mc1(&st)
                .map_err(|e| format!("import: {e}"))?;
            // Measured planes AFTER the import (the importer's
            // terrain replay double-applies on measured planes — the
            // replay verifier's proven order).
            if let Some((h, ty, ceil, an)) = self.timg.as_ref().and_then(|i| i.measured()) {
                w.install_measured_terrain(h, ty, ceil, an)
                    .map_err(|e| format!("terrain: {e}"))?;
            }
            let (fl, fr) = recover::mc1_fire(st.wizards[st.local_player as usize].move_bits);
            w.set_prev_fire(fl, fr);
            w.terrain_dirty = true;
            w.entities_dirty = true;
            self.human_slot = report.human_slot;
            sim.carpet = conformance::mc1_state_from_retail(&st, report.human_slot);
            sim.carpet_mc2 = Mc2Ext::default();
            sim.clear_accel_edge();
            sim.sync_flyer_from_carpet();
            self.segments += 1;
            self.anchored_flag = true;
            self.update_ghost_mc1(&st);
            self.prev = Some((tick.t, RetailPrev::Mc1(Box::new(st))));
            return Ok(None);
        }
        let (_, prev) = self.prev.take().expect("anchored");
        let RetailPrev::Mc1(pst) = prev else {
            unreachable!("family-stable stream")
        };
        let obs = tick
            .obs
            .as_ref()
            .and_then(|v| serde_json::from_str::<ObsMc1>(v.get()).ok());
        let rp = recover::recover_pair_mc1(&pst, &st, tick.input.as_ref());
        // ⭐⭐ THE PAUSED TURN CONSUMES A BOUNDARY WITHOUT A SIM STEP.
        // sub_41780's first statement is the global LCG draw; its
        // SECOND is `if (+2 & 1) goto LABEL_52`, which jumps the whole
        // 1..999 entity walk (:52223-25 → :52418; hw :48273-75 →
        // :48468). A paused frame therefore advances the RNG and
        // nothing else.
        //
        // `mgc-conform replay` has modelled this since the mc1l6 dig
        // (replay.rs, `paused_turn_mc1`); THIS driver did not, and ran
        // a full `sim.step` per paused boundary. mc1hwl0 pauses for 10
        // frames at t=30897..30906 (slot 472 frozen on every lane,
        // free stack unchanged), so the app arrived at t=30907 holding
        // retail's t=30917 — `tick_ctr` 155 against 145, with y and z
        // bit-identical to retail ten ticks later. It reported as a
        // `pose.x` head only because x is the first lane in the pose
        // projection.
        //
        // The pause screen is still INTERACTIVE (the big map / spell-
        // book re-equips under it), so the recovered hand change is
        // applied here exactly as the conform twin does — dropping it
        // cost mc1l6 one lane forever.
        if obs
            .as_ref()
            .is_some_and(|o| recover::paused_turn_mc1(&pst, o))
        {
            let w = sim.world.as_mut().ok_or("no world")?;
            w.tick_paused();
            if rp.equip_left.is_some() || rp.equip_right.is_some() {
                w.equip_hands(rp.equip_left.map(SpellId), rp.equip_right.map(SpellId));
            }
            self.paused += 1;
            self.update_ghost_mc1(&st);
            self.prev = Some((tick.t, RetailPrev::Mc1(Box::new(st))));
            return Ok(None);
        }
        if !rp.stick_ok() {
            self.stick_unrec += 1;
        }
        // The dw==48 strafe-freeze emulation (law in RecoveredPair).
        if rp.mc1_strafe_freeze() && sim.carpet.strafe != 0 {
            sim.carpet.strafe += 4 * sim.carpet.strafe.signum();
        }
        let input = FlightInput {
            stick_x: rp.stick().0,
            stick_y: rp.stick().1,
            fire_left: rp.fire_left,
            fire_right: rp.fire_right,
            equip_left: rp.equip_left.map(SpellId),
            equip_right: rp.equip_right.map(SpellId),
            respawn: rp.respawn,
            cheat: rp.cheat,
            demolish: rp.demolish,
            mc1_move_byte: Some(rp.move_byte as u8),
            ..FlightInput::default()
        };
        // Gradeability decided from the recording alone; the pose
        // compare itself waits for the step (`grade`).
        let gradeable = obs.is_some_and(|obs| recover::capture_clean_mc1(&pst, &obs));
        self.pending = Some(gradeable);
        self.steps += 1;
        self.update_ghost_mc1(&st);
        self.prev = Some((tick.t, RetailPrev::Mc1(Box::new(st))));
        Ok(Some(input))
    }

    fn retail_tick_mc2(
        &mut self,
        sim: &mut Simulation,
        tick: &mgcr::TickRecord,
        st: RetailMc2,
    ) -> Result<Option<FlightInput>, String> {
        // The respawn witness folds EVERY state-bearing record in
        // stream order (dating law in mgc_formats::recover).
        let respawn = self.witness.observe(tick.input.as_ref());
        let forced = std::mem::take(&mut self.force_anchor);
        let anchor =
            forced || !matches!(&self.prev, Some((pt, RetailPrev::Mc2(_))) if tick.t == pt + 1);
        if anchor {
            if forced {
                // No `reload_thing_table` twin here: the MC2 anchor
                // already restores the PRISTINE table below, which is
                // the reload's own semantics.
                self.resyncs += 1;
            }
            let w = sim.world.as_mut().ok_or("no world")?;
            w.restore_planes(self.pristine.as_ref().expect("retail install"));
            w.restore_thing_table(self.things.as_ref().expect("mc2 install"));
            // ⭐ THE CAMPAIGN REPLAY GATE IS PART OF THE TAKE'S WORLD.
            // `build_world_mc2` hands it to the conformance runner at
            // construction; the app builds its world through the level
            // loader, which has no capture to read, so the driver must
            // stamp it here — otherwise every MC2 take runs UNGATED and
            // the (14,5) XP scrolls leak for the rest of the run.
            w.set_mc2_level_replayed(self.mc2_replayed);
            let report = w
                .retail_import_mc2(&st)
                .map_err(|e| format!("import: {e}"))?;
            if let Some((h, ty, ceil, an)) = self.timg.as_ref().and_then(|i| i.measured()) {
                w.install_measured_terrain(h, ty, ceil, an)
                    .map_err(|e| format!("terrain: {e}"))?;
            }
            let (fl, fr) = recover::mc1_fire(st.players[st.local_player as usize].move_bits);
            w.set_prev_fire(fl, fr);
            w.terrain_dirty = true;
            w.entities_dirty = true;
            let row = w.mc2_carpet_row();
            self.human_slot = report.human_slot;
            let (s, ext) = conformance::mc2_state_from_retail(&st, report.human_slot, row);
            sim.carpet = s;
            sim.carpet_mc2 = ext;
            sim.clear_accel_edge();
            sim.sync_flyer_from_carpet();
            self.segments += 1;
            self.anchored_flag = true;
            self.update_ghost_mc2(&st);
            self.prev = Some((tick.t, RetailPrev::Mc2(Box::new(st))));
            return Ok(None);
        }
        let (_, prev) = self.prev.take().expect("anchored");
        let RetailPrev::Mc2(pst) = prev else {
            unreachable!("family-stable stream")
        };
        let rp = recover::recover_pair_mc2(&pst, &st, respawn, tick.input.as_ref());
        // The MC2 twin of the paused turn (see `retail_tick_mc1`):
        // same `+2 & 1` arm, phase field `+0x3E` instead of `+63`.
        if recover::paused_turn_mc2(&pst, &st) {
            let w = sim.world.as_mut().ok_or("no world")?;
            w.tick_paused();
            self.paused += 1;
            self.update_ghost_mc2(&st);
            self.prev = Some((tick.t, RetailPrev::Mc2(Box::new(st))));
            return Ok(None);
        }
        if !rp.stick_ok() {
            self.stick_unrec += 1;
        }
        let input = FlightInput {
            stick_x: rp.stick().0,
            stick_y: rp.stick().1,
            fire_left: rp.fire_left,
            fire_right: rp.fire_right,
            mc2_select: rp.mc2_select,
            spell_ring: rp.spell_ring,
            respawn: rp.respawn,
            demolish: rp.demolish,
            cheat: rp.cheat,
            // Retail's dw_0 bit 0x80 IS the barrel-roll command.
            barrel_roll: rp.move_byte & 0x80 != 0,
            mc1_move_byte: Some(rp.move_byte as u8),
            mc2_cmd_speed: None,
            mc2_park: rp.mc2_park,
            ..FlightInput::default()
        };
        let gradeable = recover::capture_clean_mc2(&pst, &st);
        self.pending = Some(gradeable);
        self.steps += 1;
        self.update_ghost_mc2(&st);
        self.prev = Some((tick.t, RetailPrev::Mc2(Box::new(st))));
        Ok(Some(input))
    }

    /// Post-step boundary grade (retail takes): the chained carpet vs
    /// the recorded pose, the pose channel's lane set. Reported, never
    /// corrected.
    pub fn grade(&mut self, sim: &Simulation) {
        let Some(gradeable) = self.pending.take() else {
            return;
        };
        let Some((t, prev)) = &self.prev else { return };
        let t = *t;
        if !gradeable {
            self.skipped += 1;
            self.refresh_hud(t);
            return;
        }
        // The full lane set, so the microscope and the grader cannot
        // disagree about what "the pose channel" is; `lanes` below is
        // this filtered to the parting lanes.
        let (all, extras) = match prev {
            RetailPrev::Mc1(st) => (
                conformance::pose_all_mc1(
                    &sim.carpet,
                    &st.ents[self.human_slot as usize],
                    &st.wizards[st.local_player as usize],
                ),
                Vec::new(),
            ),
            RetailPrev::Mc2(st) => {
                let e = &st.ents[self.human_slot as usize];
                (
                    conformance::pose_all_mc2(
                        &sim.carpet,
                        &sim.carpet_mc2,
                        e,
                        &st.players[st.local_player as usize],
                    ),
                    // Retail-only context: the death column's two tells
                    // (the fall accumulator and the action index).
                    vec![("f2c", e.f2c as i64), ("action45", e.action45 as i64)],
                )
            }
        };
        conformance::emit_pose_window(t, &all, &extras);
        let lanes: Vec<_> = all.into_iter().filter(|&(_, w, g)| w != g).collect();
        self.graded += 1;
        if lanes.is_empty() {
            // ⚠ CLEAN IS THE HORIZON, NOT A TALLY. It used to count
            // every clean boundary in the whole take, so mc2l4 read
            // "718 clean; DIVERGED since t=573" and the app's headline
            // number meant something different from `mgc-conform
            // --brief`'s `clean=`, which stops at the first divergence.
            // Ticks that come back clean AFTER the horizon are still
            // worth knowing (they say the divergence is transient), so
            // they are counted separately and reported as such.
            if self.diverged.is_none() {
                self.clean += 1;
            } else {
                self.clean_after += 1;
            }
        } else if self.diverged.is_none() {
            let (lane, want, got) = lanes[0];
            self.diverged = Some((t, lane.to_string()));
            println!("replay: first pose divergence t={t} — {lane}: retail {want} port {got}");
        }
        self.refresh_hud(t);
    }

    fn update_ghost_mc1(&mut self, st: &RetailMc1) {
        let e = &st.ents[usize::from(self.human_slot)];
        self.ghost = Some((
            e.x as f32 / 256.0,
            e.z as f32 / 256.0,
            e.y as f32 / 256.0,
            (e.f30 & 0x7FF) as f32 * GHOST_RAD,
            e.type86,
        ));
    }

    fn update_ghost_mc2(&mut self, st: &RetailMc2) {
        let e = &st.ents[usize::from(self.human_slot)];
        self.ghost = Some((
            e.x as f32 / 256.0,
            e.z as f32 / 256.0,
            e.y as f32 / 256.0,
            (e.yaw as u16 & 0x7FF) as f32 * GHOST_RAD,
            // The MC2 wizard-carpet art family, human color row —
            // row 44, NOT 272 (that one is the storm cloud; see
            // `mc2::carpet_sprite_row`).
            mgc_sim::mc2::carpet_sprite_row(0),
        ));
    }
}

// ------------------------------------------------------------ recorder

/// The port recorder (`--record`): `source:"port"`, `input:"exact"`,
/// hash channel on, the start state embedded as `start_mgcs_b64` (so
/// mid-level/campaign starts replay exactly — a pristine level boot
/// is just the t=0 special case of the same law).
pub struct PortRecorder {
    w: RecordingWriter,
    path: PathBuf,
}

impl PortRecorder {
    /// `pool_slots`/`awake_range` are the OFFLINE chassis overrides the
    /// session was launched with (`None` = the faithful default). They
    /// have to ride the header because they are decided BEFORE the
    /// world exists: the replay must build its world at the recorded
    /// size, and only then apply `start_mgcs_b64`. They are implicit in
    /// the snapshot too — `Gen::snap_check_identity` opens on
    /// `chassis.pool_slots` and `chassis.awake_gate_sq` — but that is
    /// the REFUSAL, reached far too late to configure anything, which
    /// is exactly how a take made with `--pool-slots N` became
    /// unreplayable ("snapshot is for a different world").
    pub fn begin(
        path: &Path,
        sim: &Simulation,
        game: &str,
        level: u32,
        thrust: ThrustModel,
        altitude: AltitudeModel,
        pool_slots: Option<usize>,
        awake_range: Option<u32>,
        pin: ImportPin,
    ) -> Result<PortRecorder, String> {
        let mut header = serde_json::json!({
            "format": 1,
            "game": game,
            "level": level,
            "source": "port",
            "tick_hz": mgc_sim::TICK_RATE_HZ,
            "channels": {"input": "exact", "obs": false, "state": false, "hash": true},
            "build": env!("CARGO_PKG_VERSION"),
            "tool": {"name": "mgcarpet --record", "version": env!("CARGO_PKG_VERSION")},
            "sim": {
                "snapshot_version": mgc_sim::snapshot::SNAPSHOT_VERSION,
                "thrust_model": match thrust {
                    ThrustModel::Mc1 => "classic",
                    ThrustModel::Enhanced => "enhanced",
                },
                "altitude_model": match altitude {
                    AltitudeModel::Faithful => "classic",
                    AltitudeModel::ExtendedLift => "enhanced",
                },
                // The retail-bug patch policy: --record forces every
                // patch to its retail arm for the whole session, and
                // says so here. A header WITHOUT this key is a take
                // from before the patches became options — it replays
                // under the legacy hard-wired set (ReplayFile::
                // patch_policy).
                "patches": "retail",
                "start_mgcs_b64": mgcr::b64_encode(&sim.snapshot()),
            },
        });
        // Written only when overridden, so a faithful take keeps the
        // exact header shape it had before these keys existed.
        if let Some(sim) = header["sim"].as_object_mut() {
            if let Some(n) = pool_slots {
                sim.insert("entity_pool_size".into(), serde_json::json!(n));
            }
            if let Some(n) = awake_range {
                sim.insert("awake_range".into(), serde_json::json!(n));
            }
            // The import pin (`World::import_pin`): CONFIG the
            // snapshot deliberately skips, so a take whose start state
            // came from a retail import has to carry it or its replay
            // takes the deviating arms and desyncs against its own
            // hash channel. Omitted entirely for a native session,
            // whose pin is all-default.
            if pin != ImportPin::default() {
                sim.insert(
                    "import_pin".into(),
                    serde_json::json!({
                        "strict_retail": pin.strict_retail,
                        "measured_terrain": pin.measured_terrain,
                        "carpet_slot": pin.carpet_slot,
                        "castle_reg": pin.castle_reg,
                        "human_pose_prev": [
                            pin.human_pose_prev.0,
                            pin.human_pose_prev.1,
                            pin.human_pose_prev.2,
                        ],
                        "human_yaw": pin.human_yaw,
                        "human_yaw_prev": pin.human_yaw_prev,
                        "mc1_hand_bits": pin.hand_bits,
                        "mc1_cast_pose": [
                            pin.mc1_cast_pose.x as i64,
                            pin.mc1_cast_pose.y as i64,
                            pin.mc1_cast_pose.z as i64,
                            pin.mc1_cast_pose.heading as i64,
                            pin.mc1_cast_pose.pitch as i64,
                            pin.mc1_cast_pose.speed as i64,
                        ],
                        "mc1_acq": pin.mc1_acq,
                        "mc2_turn": pin.mc2_turn,
                        "mc2_carpet_stall": pin.mc2_carpet_stall,
                        "mc1_frame": pin.mc1_frame,
                        "mc1_hud_drawn": pin.mc1_hud_drawn,
                        "mc1_v14": pin.mc1_v14,
                        "accel_veto": [pin.accel_veto.0, pin.accel_veto.1],
                        "pending_teleport": pin.pending_teleport
                            .map(|(x, y, z)| serde_json::json!([x, y, z])),
                        "pending_respawn": pin.pending_respawn
                            .map(|(x, y, z)| serde_json::json!([x, y, z])),
                        "pending_restart": pin.pending_restart,
                        "wiz_charge": pin.wiz_charge,
                    }),
                );
            }
        }
        Ok(PortRecorder {
            w: RecordingWriter::create(path, &header)?,
            path: path.to_owned(),
        })
    }

    /// One tick: `t`/`hash` describe the PRE-step state, `input` is
    /// what the step consumed (the phase convention). `set` carries
    /// any live-option events applied since the previous row — the
    /// hash already includes their effect, so a replayer applies them
    /// before grading (see [`mgcr::TickRecord::set`]).
    pub fn record(
        &mut self,
        t: u64,
        input: &FlightInput,
        hash: u64,
        set: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut row = serde_json::json!({
            "t": t,
            "input": port_input_from(input),
            "hash": format!("{hash:016x}"),
        });
        if !set.is_empty()
            && let Some(o) = row.as_object_mut()
        {
            o.insert("set".into(), serde_json::Value::Object(set.clone()));
        }
        self.w.write_record(&row)
    }

    pub fn finish(self) -> (PathBuf, u64, Result<(), String>) {
        let n = self.w.records();
        (self.path, n, self.w.finish())
    }
}

fn port_input_from(i: &FlightInput) -> PortInput {
    PortInput {
        thrust: i.thrust,
        strafe: i.strafe,
        lift: i.lift,
        yaw_delta: i.yaw_delta,
        pitch_delta: i.pitch_delta,
        stick_x: i.stick_x,
        stick_y: i.stick_y,
        fire_left: i.fire_left,
        fire_right: i.fire_right,
        equip_left: i.equip_left.map(|s| s.0),
        equip_right: i.equip_right.map(|s| s.0),
        mc2_select: i.mc2_select,
        spell_ring: i.spell_ring,
        full_stop: i.full_stop,
        respawn: i.respawn,
        demolish: i.demolish,
        barrel_roll: i.barrel_roll,
        raw_dx: i.raw_dx,
        mc1_move_byte: i.mc1_move_byte,
        mc2_cmd_speed: i.mc2_cmd_speed,
        mc2_park: i.mc2_park,
        suicide: i.suicide,
        cheat: i.cheat.map(|c| c.code()),
    }
}

fn flight_input_from(p: &PortInput) -> FlightInput {
    FlightInput {
        thrust: p.thrust,
        strafe: p.strafe,
        lift: p.lift,
        yaw_delta: p.yaw_delta,
        pitch_delta: p.pitch_delta,
        stick_x: p.stick_x,
        stick_y: p.stick_y,
        fire_left: p.fire_left,
        fire_right: p.fire_right,
        equip_left: p.equip_left.map(SpellId),
        equip_right: p.equip_right.map(SpellId),
        mc2_select: p.mc2_select,
        spell_ring: p.spell_ring,
        full_stop: p.full_stop,
        respawn: p.respawn,
        demolish: p.demolish,
        // Retail's own captures cannot carry these three — suicide is
        // a direct life write in the key handler, and the MC2 cruise
        // lanes come out of the RECOVERY, not the raw channel. A port
        // take can: `--record` under `--replay` writes the recovered
        // input, so the stream has to round-trip every field the
        // driver sets or an MC2 re-record cannot reproduce itself.
        suicide: p.suicide,
        barrel_roll: p.barrel_roll,
        raw_dx: p.raw_dx,
        mc1_move_byte: p.mc1_move_byte,
        mc2_cmd_speed: p.mc2_cmd_speed,
        mc2_park: p.mc2_park,
        cheat: p.cheat.and_then(mgc_formats::recover::Cheat::from_code),
    }
}

/// Apply one row's recorded live-option events (`TickRecord::set`) —
/// the replay half of the port toggle channel. Each key routes to the
/// SAME setter the app's options menu calls (`apply_option`), which is
/// what makes record and replay agree by construction; these are port
/// constructs and deliberately NOT the retail cheat lane, whose
/// opcodes run retail's own (subtly different) semantics.
fn apply_live_options(
    sim: &mut Simulation,
    set: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    for (key, v) in set {
        let b = || {
            v.as_bool()
                .ok_or_else(|| format!("live option {key:?} wants a bool, got {v}"))
        };
        match key.as_str() {
            "invincible" => sim
                .world
                .as_mut()
                .ok_or("live option needs a world")?
                .set_invincible(b()?),
            "dev_spells" => sim
                .world
                .as_mut()
                .ok_or("live option needs a world")?
                .set_dev_spells(b()?),
            "ghost" => sim
                .world
                .as_mut()
                .ok_or("live option needs a world")?
                .set_ghost(b()?),
            "inert" => sim
                .world
                .as_mut()
                .ok_or("live option needs a world")?
                .set_inert(b()?),
            // `lift_unclamped` is the pre-2026-09-15 name of the same
            // toggle; old port takes still carry it.
            "weightless" | "lift_unclamped" => sim.weightless = b()?,
            "thrust_model" => {
                sim.set_thrust_model(match v.as_str() {
                    Some("classic") => ThrustModel::Mc1,
                    Some("enhanced") => ThrustModel::Enhanced,
                    other => return Err(format!("unknown thrust_model {other:?}")),
                });
            }
            "altitude_model" => {
                sim.set_altitude_model(match v.as_str() {
                    Some("classic") => AltitudeModel::Faithful,
                    Some("enhanced") => AltitudeModel::ExtendedLift,
                    other => return Err(format!("unknown altitude_model {other:?}")),
                });
            }
            other => {
                return Err(format!(
                    "take sets live option {other:?} this build does not implement — refusing"
                ));
            }
        }
    }
    Ok(())
}

// -------------------------------------------------------- replay-check

/// Headless verification playback (`--replay-check`): the whole take
/// through the app's own `Simulation::step` path, no window — the
/// certification instrument for the in-app replay chain (its retail
/// results must match `mgc-conform replay`'s, which certifies the
/// integer-pose faithful path end to end). Exit truth: `Ok(true)` =
/// zero divergence.
/// `record` is the headless twin of `--replay --record`: batch
/// conversion without a window, on the same two rules
/// ([`begin_replay_recording`]).
pub fn replay_check(
    mut level: LoadedLevel,
    mut file: ReplayFile,
    record: Option<&Path>,
) -> Result<bool, String> {
    let game = level.game;
    let lvl = file.level;
    // The source take's offline chassis params, forwarded into a
    // re-recording's header — the world stepping here was built at
    // exactly these (the boot pinned cfg from the take), and a new
    // header without them embeds a snapshot its own replay would
    // refuse ("chassis.pool_slots differs"). Port takes only; a
    // retail take runs the faithful chassis.
    let (src_pool, src_awake) = if file.source == ReplaySource::Port {
        (file.sim_pool_slots, file.sim_awake_range)
    } else {
        (None, None)
    };
    let w = level.world.take().ok_or("level built no world")?;
    let mut sim = Simulation::with_world(w);
    let (thrust, altitude) = file.models()?;
    sim.thrust_model = thrust;
    sim.altitude_model = altitude;
    if let Some(start) = level.start {
        sim.flyer = start;
        sim.sync_carpet_from_flyer();
    }
    if let Some(snap) = file.snapshot.take() {
        sim.restore(&snap).map_err(|e| format!("snapshot: {e}"))?;
    }
    let mut d = ReplayDriver::install(file, &mut sim)?;
    let mut rec: Option<PortRecorder> = None;
    let mut cut_at = None;
    while let Some(input) = d.next(&mut sim) {
        if let Some(path) = record {
            let anchored = d.take_anchored();
            match &rec {
                // Rule 1: the world only becomes the take's world once
                // the driver has anchored, which happens inside the
                // first `next`.
                None => {
                    rec = Some(begin_replay_recording(
                        path, &sim, game, lvl, src_pool, src_awake,
                    )?);
                }
                // A LATER anchor is a capture gap the take healed by
                // re-seeding from the next closure. Input alone cannot
                // cross a gap — the missing ticks are missing — so the
                // recording ends here rather than emitting a stream
                // whose own hashes it could not reproduce.
                Some(_) if anchored => {
                    cut_at = Some(sim.tick);
                    break;
                }
                Some(_) => {}
            }
        }
        // The `--record` phase convention: t/hash describe the state
        // the step is ABOUT to consume, `input` is what it consumed.
        // The row's live-option events came from the driver (it just
        // applied them, so this pre-step hash includes their effect)
        // and are carried into the re-recording verbatim.
        let pre = rec
            .as_ref()
            .map(|_| (sim.tick, sim.state_hash(), d.take_row_set()));
        sim.step(&input);
        d.grade(&sim);
        if let (Some(r), Some((t, hash, set))) = (rec.as_mut(), pre) {
            r.record(t, &input, hash, &set)?;
        }
        // The windowed session drains the sim's sound requests every
        // tick (`audio_tick` → `World::take_audio`) and that vec is
        // HASHED — a take recorded windowed carries post-drain hashes,
        // so the headless check must drain on the same cadence or it
        // desyncs at t=1 on accumulated sounds alone.
        let f = &sim.flyer;
        let pose =
            mgc_sim::engine::world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, 0.0);
        if let Some(w) = sim.world.as_mut() {
            let _ = w.take_audio(pose);
        }
    }
    println!("replay-check: {}", d.summary());
    if let Some(r) = rec {
        let (path, n, res) = r.finish();
        res?;
        println!(
            "record: {} — {n} tick(s), input-only port take",
            path.display()
        );
        if let Some(t) = cut_at {
            println!(
                "record: STOPPED at t={t} — the take re-anchors there (capture \
                 gap); input alone cannot cross a gap, so only the first \
                 segment was recorded"
            );
        }
    }
    Ok(d.diverged.is_none())
}

/// Begin a port recording for a session that is REPLAYING one.
///
/// `--record` under `--replay` was refused outright, which left no way
/// to turn a retail take into something shareable. A retail take is
/// ~500 KB/tick and nearly all of it is the two channels the replay
/// does not hand to the sim: `state` (61%) and `obs` (39%). The
/// tick's player input is 0.0% of the file — and it is not the `input`
/// channel either, which holds the raw DOSBox externals
/// (`keys_down`/`mouse`) that retail's consume loop filters and
/// latches before the mover sees them. The replayable signal is
/// RECOVERED from the state closure pair by `mgc_formats::recover`.
/// So the crop cannot be a channel filter — but a replay ALREADY
/// recovers that input every tick, and `--record` already writes
/// exactly the input-only format. Letting them meet is the whole
/// feature.
///
/// TWO RULES make the result reproduce, and both are why this is not
/// simply `PortRecorder::begin` at session start:
///
/// 1. START AFTER THE ANCHOR. A retail take seeds the world from its
///    first closure INSIDE the driver's first `next` call, so a
///    snapshot taken at session install captures a pre-seed world and
///    the take desyncs against its own hash channel on tick one.
/// 2. CARRY THE IMPORT PIN. The seeding is an import, and
///    [`ImportPin`] is the part of an imported world a snapshot
///    deliberately does not hold.
pub fn begin_replay_recording(
    path: &Path,
    sim: &Simulation,
    game: mgc_sim::ids::GameId,
    level: u32,
    pool_slots: Option<usize>,
    awake_range: Option<u32>,
) -> Result<PortRecorder, String> {
    PortRecorder::begin(
        path,
        sim,
        match game {
            mgc_sim::ids::GameId::Mc1 => "mc1",
            mgc_sim::ids::GameId::Mc1Hw => "mc1hw",
            mgc_sim::ids::GameId::Mc2 => "mc2",
        },
        level,
        sim.thrust_model,
        sim.altitude_model,
        pool_slots,
        awake_range,
        sim.world
            .as_ref()
            .map(|w| w.import_pin())
            .unwrap_or_default(),
    )
}

/// The ghost's translucent billboard, if the session can resolve the
/// sprite (see `entities::ghost_billboard`).
pub fn ghost_billboard(
    driver: &ReplayDriver,
    game: mgc_sim::ids::GameId,
    sprite_dims: &impl Fn(u16) -> Option<(u16, u16, u16)>,
) -> Option<mgc_render::Billboard> {
    let (x, alt, z, yaw, type_index) = driver.ghost?;
    crate::entities::ghost_billboard(game, type_index, x, alt, z, yaw, sprite_dims)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-record MC1 retail stub, wired straight into the driver's
    /// retail arm — no take, no level, no gamedata.
    fn paused_stub_driver() -> (ReplayDriver, Simulation, RetailMc1) {
        // A header-only .mgcr: the driver never pulls from it here
        // (the test calls `retail_tick_mc1` directly), but `Recording`
        // is only constructible from a stream.
        // ⚠ UNIQUE PER CALL. This was a fixed filename until the
        // resync pins landed; with four tests sharing one path,
        // `Recording::open` can read a file another thread is still
        // writing. A shared temp path is a race that only shows up
        // once a second test uses the helper.
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mgcapp-paused-stub-{n}.mgcr"));
        std::fs::write(
            &path,
            b"{\"format\":2,\"game\":\"mc1\",\"source\":\"retail\",\"tick_hz\":25,\
              \"channels\":{\"input\":\"raw\",\"obs\":true,\"state\":true,\"hash\":false}}\n",
        )
        .expect("stub header");
        let rec = Recording::open(&path).expect("stub opens");

        // A world is needed only because `tick_paused` lives on one.
        let planes = Planes {
            height: vec![100; 0x10000],
            tile_type: vec![5; 0x10000],
            shading: vec![32; 0x10000],
            angle: vec![5; 0x10000],
            ceiling: Vec::new(),
        };
        // Diamond rings, like `features::tests::synthetic_assets`.
        let mut grid = vec![31u8; 1024];
        for y in 0..32i32 {
            for x in 0..32i32 {
                let (dx, dy) = (x - 15, y - 15);
                let r = dx.max(dy).max(-dx + 1).max(-dy + 1) - 1;
                grid[(y * 32 + x) as usize] = r.clamp(0, 31) as u8;
            }
        }
        let assets = mgc_sim::engine::features::FeatureAssets::parse(
            &grid,
            &(0..24u32)
                .flat_map(|_| {
                    let mut e = 0u32.to_le_bytes().to_vec();
                    e.extend_from_slice(&[4, 4]);
                    e
                })
                .collect::<Vec<u8>>(),
            &[4, 0x10, 0x10, 0x10, 0x10, 0],
        )
        .expect("synthetic assets");
        let sim =
            Simulation::with_world(mgc_sim::engine::world::World::new(planes, &[], 1, assets));

        // The previous retail closure: two live slots, one wizard.
        let mut pst = RetailMc1 {
            rand: 0x1234_5678,
            wizards: vec![mgcr::RetailWizardMc1::default()],
            ents: vec![mgcr::RetailEntMc1::default(); 8],
            ..RetailMc1::default()
        };
        for (slot, (class, model)) in [(1usize, (5u8, 2u8)), (2, (4, 13))] {
            pst.ents[slot].class64 = class;
            pst.ents[slot].model65 = model;
            pst.ents[slot].rand = 0x0BAD_F00D;
            pst.ents[slot].f63 = 7;
        }
        let driver = ReplayDriver {
            rec,
            source: ReplaySource::Retail,
            family: Family::Mc1,
            timg: None,
            mc2_replayed: false,
            pristine: None,
            things: None,
            witness: recover::Mc2RespawnWitness::default(),
            prev: Some((100, RetailPrev::Mc1(Box::new(pst.clone())))),
            human_slot: 0,
            pending: None,
            anchored_flag: false,
            force_anchor: false,
            segments: 1,
            resyncs: 0,
            steps: 0,
            graded: 0,
            clean: 0,
            clean_after: 0,
            skipped: 0,
            stick_unrec: 0,
            paused: 0,
            diverged: None,
            hud: String::new(),
            ghost: None,
            row_set: serde_json::Map::new(),
            finished: false,
        };
        (driver, sim, pst)
    }

    /// The `obs` channel for the tick after `pst`: one LCG step and
    /// every live slot frozen is a PAUSE; `bump` perturbs one entity's
    /// phase byte to make it an ordinary turn instead.
    /// The stub obs as RAW JSON — `TickRecord::obs` is the recording's
    /// obs TEXT now, not a materialised `Value`.
    fn stub_obs(pst: &RetailMc1, bump: bool) -> Box<serde_json::value::RawValue> {
        serde_json::value::to_raw_value(&stub_obs_value(pst, bump)).expect("stub obs")
    }

    fn stub_obs_value(pst: &RetailMc1, bump: bool) -> serde_json::Value {
        let rng = pst.rand.wrapping_mul(9377).wrapping_add(9439);
        let ents: Vec<serde_json::Value> = [1usize, 2]
            .iter()
            .map(|&slot| {
                let e = &pst.ents[slot];
                serde_json::json!({
                    "slot": slot,
                    "class": e.class64,
                    "model": e.model65,
                    "sclass": 0,
                    "smodel": 0,
                    "flags": 0,
                    "id": 0,
                    "life": 100,
                    "max_life": 100,
                    "x": 100.0,
                    "y": 100.0,
                    "z": 0,
                    "heading": 0,
                    "pitch": 0,
                    "target_yaw": 0,
                    "speed": 0,
                    "mana": 0,
                    "mana_max": 0,
                    "chase": 0,
                    "owner_ptr": 0,
                    "tick_byte": if bump && slot == 2 { e.f63 + 1 } else { e.f63 },
                    "rand": e.rand,
                })
            })
            .collect();
        let v = serde_json::json!({
            "rng": rng,
            "n_active": 2,
            "local_player": 0,
            "player_count": 1,
            "wizards": [],
            "control": [],
            "player": null,
            "entities": ents,
        });
        // ⚠ A stub the driver cannot DECODE makes both pins below
        // vacuous — the paused branch is gated on `obs` being Some, so
        // a missing channel would read as "not a pause" and step. Fail
        // here instead, loudly, when the projection grows a field.
        ObsMc1::deserialize(&v).expect("the stub obs must decode — a new channel needs a value");
        v
    }

    /// ⭐⭐ THE PAUSED TURN CONSUMES A BOUNDARY WITHOUT A SIM STEP.
    /// `sub_41780`'s second statement (`if (+2 & 1) goto LABEL_52`,
    /// :52224-25 → :52418) jumps the whole 1..999 entity walk, so a
    /// paused frame advances the global LCG and nothing else.
    /// `mgc-conform replay` has modelled it since the mc1l6 dig; THIS
    /// driver ran a full `sim.step` per paused boundary until session
    /// 69, which left it holding retail's state TEN TICKS LATE across
    /// mc1hwl0's 10-frame pause (app horizon 30,896 → 32,879; mc1l6
    /// app 0 → 4,064 over its 1,602 paused boundaries).
    ///
    /// The pin is the CONTROL FLOW, which the corpus alone guards:
    /// a paused pair must yield no input and no step.
    #[test]
    fn the_app_driver_spends_a_paused_boundary_without_a_sim_step() {
        let (mut d, mut sim, pst) = paused_stub_driver();
        let before = (sim.tick, sim.state_hash());
        let tick = mgcr::TickRecord {
            t: 101,
            obs: Some(stub_obs(&pst, false)),
            ..mgcr::TickRecord::default()
        };
        let mut st = pst.clone();
        st.rand = pst.rand.wrapping_mul(9377).wrapping_add(9439);

        let out = d.retail_tick_mc1(&mut sim, &tick, st).expect("paused turn");
        assert!(
            out.is_none(),
            "a paused boundary yields NO flight input — the caller must \
             pull the next record, not step the sim"
        );
        assert_eq!(d.paused, 1, "and it is booked as paused");
        assert_eq!(
            d.steps, 0,
            "⚠ REVERSION PROBE: dropping the branch \
             books a step here and desyncs by one tick per paused frame"
        );
        assert_eq!(sim.tick, before.0, "the sim clock does not advance");
        assert_ne!(
            sim.state_hash(),
            before.1,
            "but the world DID take its paused turn — the global LCG \
             draw is the one thing sub_41780 does before the jump"
        );
    }

    /// ⭐ THE PERMADEATH RESYNC (player-ruled 2026-09-07): an ARMED
    /// restart turns the very next boundary into an ANCHOR, even
    /// though the stream is unbroken (`t == pt + 1`).
    ///
    /// The pin is the control flow, because the alternative is
    /// silent and catastrophic: before this, a castle-less death
    /// under `--replay` ran `App::restart_level`, which throws
    /// retail's imported world away and builds the PORT's own native
    /// level init in its place — a different world (mc1l48 at its own
    /// phase: 942 live / 22 families against retail's 943 / 40). The
    /// take detonated on the spot: mc1l48's app horizon was 8,591,
    /// the reload seam, and everything past it was noise. With the
    /// resync the conform twin free-runs t=8591..9180 at 588 of 588
    /// graded boundaries CLEAN, and the take's first real divergence
    /// becomes the castle painter at 9,786 — the same head
    /// `--segmented` has always reported.
    ///
    /// The flag is ONE-SHOT: it must not turn the rest of the take
    /// into a re-anchoring segmented run behind the player's back.
    #[test]
    fn an_armed_restart_anchors_the_next_boundary() {
        let (mut d, mut sim, pst) = paused_stub_driver();
        d.pristine = Some(sim.world.as_ref().expect("stub world").planes_clone());
        // The import needs a real human CARPET (class 3) at the
        // wizard's `play_index`; the paused-turn stub never anchors,
        // so it leaves the index at 0 and grows no carpet.
        let mut pst = pst;
        // ...and a full 8-wizard table: the import seats every rival
        // slot (`rival_ents`/`castle_reg`), where the paused-turn
        // stub only ever needed the local one.
        pst.wizards.resize(8, mgcr::RetailWizardMc1::default());
        pst.ents[3].class64 = 3;
        pst.ents[3].model65 = 0;
        pst.ents[3].f63 = 7;
        pst.wizards[0].play_index = 3;
        d.prev = Some((100, RetailPrev::Mc1(Box::new(pst.clone()))));
        // A pair that would otherwise STEP (the negative control's
        // own perturbed phase byte), so the anchor cannot be
        // confused with the paused-turn arm.
        let tick = mgcr::TickRecord {
            t: 101,
            obs: Some(stub_obs(&pst, true)),
            ..mgcr::TickRecord::default()
        };
        let mut st = pst.clone();
        st.ents[1].f63 = 8;
        let (segs, steps) = (d.segments, d.steps);

        d.arm_resync();
        let out = d
            .retail_tick_mc1(&mut sim, &tick, st.clone())
            .expect("armed resync anchors");
        assert!(
            out.is_none(),
            "⚠ REVERSION PROBE: an armed restart must ANCHOR, not step —              stepping here is the old `restart_level` detonation in a              different costume"
        );
        assert_eq!(d.segments, segs + 1, "the anchor opens a segment");
        assert_eq!(d.resyncs, 1, "and books itself as a permadeath resync");
        assert_eq!(d.steps, steps, "an anchor is not a step");
        assert!(
            d.take_anchored(),
            "the app is told, so it re-syncs its own view of the world"
        );

        // ONE-SHOT: the next unbroken boundary steps again.
        let tick2 = mgcr::TickRecord {
            t: 102,
            obs: Some(stub_obs(&st, true)),
            ..mgcr::TickRecord::default()
        };
        let mut st2 = st.clone();
        st2.ents[1].f63 = 9;
        let out2 = d
            .retail_tick_mc1(&mut sim, &tick2, st2)
            .expect("ordinary boundary");
        assert!(
            out2.is_some(),
            "the resync is ONE-SHOT — a plain run must not silently              become a re-anchoring segmented one"
        );
        assert_eq!(d.resyncs, 1, "and it books exactly one resync");
    }

    /// ⭐⭐ THE SPEED-DEPENDENT REGRESSION PIN (player-witnessed
    /// 2026-09-07). `next` must consume the world's restart signal
    /// ITSELF, because it is the only thing called exactly once per
    /// sim step. When the poll lived in the app's frame loop instead,
    /// a replay at 1x held through mc1l48's permadeath and the SAME
    /// replay sped up reported "DIVERGED since t=8592" — the frame
    /// ran N steps, the restart fired mid-burst, and the rest of the
    /// burst fed recorded input into an un-restarted world.
    ///
    /// The pin is that a restart raised by the PREVIOUS step is
    /// already armed by the time this boundary is dispatched.
    #[test]
    fn next_consumes_the_restart_signal_itself() {
        let (mut d, mut sim, _pst) = paused_stub_driver();
        assert!(!d.force_anchor, "nothing armed to begin with");
        sim.world
            .as_mut()
            .expect("stub world")
            .debug_signal_restart();

        // `next` pulls no record here (the stub's stream is empty),
        // but the poll must have happened on the way in.
        let _ = d.next(&mut sim);
        assert!(
            d.force_anchor,
            "⚠ REVERSION PROBE: moving this poll back into a caller's              loop makes the resync land a whole FRAME late at game              speeds above 1x — the exact shape of the t=8592 bug"
        );
        assert!(
            !sim.world.as_mut().expect("stub world").take_restart(),
            "and it CONSUMED the signal — a second arm would re-anchor              a boundary that already resynced"
        );
    }

    /// A PORT take records the port's own restart, so replaying it
    /// must go back through `restart_level` — arming the resync on
    /// one would import a retail closure the take does not have.
    #[test]
    fn a_port_take_never_arms_the_resync() {
        let (mut d, mut sim, _pst) = paused_stub_driver();
        d.source = ReplaySource::Port;
        d.arm_resync();
        assert!(
            !d.force_anchor,
            "only a RETAIL take has a closure to re-anchor on"
        );
        assert!(!d.is_retail(), "and the app routes it to restart_level");

        // ...and `next` must leave the signal STANDING for it, or the
        // app's own poll never sees it and the level never restarts.
        sim.world
            .as_mut()
            .expect("stub world")
            .debug_signal_restart();
        let _ = d.next(&mut sim);
        assert!(
            sim.world.as_mut().expect("stub world").take_restart(),
            "a port take's restart belongs to `App::restart_level`"
        );
    }

    /// The negative control on the same stub: perturb ONE entity's
    /// phase byte and the pair stops being a pause, so the driver
    /// recovers a hand and hands back an input to step with. Without
    /// this, a predicate that answered `true` for everything would
    /// pass the pin above and freeze the whole replay.
    #[test]
    fn an_ordinary_boundary_still_steps() {
        let (mut d, mut sim, pst) = paused_stub_driver();
        let tick = mgcr::TickRecord {
            t: 101,
            obs: Some(stub_obs(&pst, true)),
            ..mgcr::TickRecord::default()
        };
        let mut st = pst.clone();
        st.rand = pst.rand.wrapping_mul(9377).wrapping_add(9439);

        let out = d
            .retail_tick_mc1(&mut sim, &tick, st)
            .expect("ordinary turn");
        assert!(out.is_some(), "one moving entity is not a pause");
        assert_eq!((d.paused, d.steps), (0, 1));
    }

    /// Opening an MC2 retail take DERIVES its campaign REPLAY gate, so
    /// the app's driver can stamp the world with it exactly as
    /// `build_world_mc2` stamps the conformance runner's. The app
    /// hardcoded `false` for four sessions, and showed on mc2l3 as a
    /// mover divergence 900 ticks after the scrolls it actually leaked.
    ///
    /// ⚠ THIS TEST READS CONFORMANCE FIXTURES, NEVER `recordings/`.
    /// A recording is PROVENANCE, not identity: it can be re-cut or
    /// overwritten at any time, and on 2026-09-13 the `mc2l0` take was
    /// replaced by a fresh campaign-replay capture whose gate is SET
    /// where the old one was clear — reddening the take-identity
    /// assertion this test used to carry. The two exemplars below are
    /// committed fixtures cut at scroll-bearing ticks, so the property
    /// travels with the repo.
    #[test]
    fn an_mc2_capture_carries_its_own_replay_gate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures");
        let gate = |lvl: &str, file: &str| {
            let p = root.join(lvl).join(file);
            p.exists()
                .then(|| mgc_formats::mgcr::mc2_take_replayed(&p))
                .transpose()
                .expect("the fixture opens")
        };
        // A campaign REPLAY births its (14,5) XP scrolls at 0x0D.
        if let Some(g) = gate("mc2l3", "the-campaign-replay-gate-is-set-on-this-capture.mgcr") {
            assert!(g, "mc2l3's scrolls are born at 0x0D — the gate is SET");
        }
        // A FIRST-RUN capture births them at 0x0C. Both exemplars are
        // needed: a hardcoded `false` passes this one and fails the
        // one above, which is exactly how the old bug hid.
        if let Some(g) = gate("mc2l30", "the-campaign-replay-gate-is-clear-on-this-capture.mgcr") {
            assert!(!g, "mc2l30 is a first run — the gate is clear");
        }
    }

    /// The port loop, closed: record a world-less faithful session,
    /// reopen the file, replay it onto a fresh sim — every tick must
    /// hash-verify and the end states must agree bit for bit (the
    /// `verify-replay` contract, docs/RECORDING.md "Consumers").
    #[test]
    fn port_record_replay_roundtrip() {
        let path = std::env::temp_dir().join("mgcapp-port-roundtrip.mgcr");
        let mut a = Simulation::with_terrain(Vec::new());
        a.thrust_model = ThrustModel::Mc1;
        a.altitude_model = AltitudeModel::Faithful;
        let mut rec = PortRecorder::begin(
            &path,
            &a,
            "mc1",
            0,
            a.thrust_model,
            a.altitude_model,
            None,
            None,
            ImportPin::default(),
        )
        .unwrap();
        for t in 0..200u64 {
            let input = FlightInput {
                thrust: if t % 50 < 25 { 1.0 } else { 0.0 },
                stick_x: (t % 40) as i16 - 20,
                stick_y: 5,
                fire_left: t % 7 == 0,
                ..FlightInput::default()
            };
            rec.record(a.tick, &input, a.state_hash(), &serde_json::Map::new())
                .unwrap();
            a.step(&input);
        }
        let (_, n, res) = rec.finish();
        res.unwrap();
        assert_eq!(n, 200);

        let mut file = ReplayFile::open(&path).unwrap();
        assert_eq!(file.source, ReplaySource::Port);
        let (thrust, altitude) = file.models().unwrap();
        let mut b = Simulation::with_terrain(Vec::new());
        b.thrust_model = thrust;
        b.altitude_model = altitude;
        b.restore(&file.snapshot.take().unwrap()).unwrap();
        let mut d = ReplayDriver::install(file, &mut b).unwrap();
        let mut steps = 0;
        while let Some(input) = d.next(&mut b) {
            b.step(&input);
            d.grade(&b);
            steps += 1;
        }
        assert_eq!(steps, 200);
        assert!(d.diverged.is_none(), "{}", d.summary());
        assert_eq!(d.graded, 200);
        assert_eq!(a.state_hash(), b.state_hash());
        let _ = std::fs::remove_file(&path);
    }

    /// **THE PORT INPUT CHANNEL CARRIES EVERY FIELD THE SIM STEPS ON.**
    ///
    /// `PortInput` is the serialization mirror of `FlightInput`, and a
    /// field missing from it is silently dropped by every port take.
    /// Three were: `mc2_cmd_speed` and `mc2_park` — which the MC2
    /// recovery sets on EVERY pair, so no MC2 take could reproduce
    /// itself — and `suicide`, which a live `--record` session lost the
    /// moment the player pressed Shift+K. Found by re-recording a
    /// replay, where a dropped lane shows up immediately as a take that
    /// desyncs against its own hash channel.
    ///
    /// Guard shape: build a `FlightInput` with EVERY field non-default,
    /// round-trip it, and compare field by field. A new `FlightInput`
    /// field fails this the moment it is stepped on and not mirrored.
    #[test]
    fn port_input_round_trips_every_flight_input_field() {
        let want = FlightInput {
            thrust: 0.25,
            strafe: -0.5,
            lift: 0.75,
            yaw_delta: 1.5,
            pitch_delta: -2.5,
            stick_x: 11,
            stick_y: -22,
            fire_left: true,
            fire_right: true,
            equip_left: Some(SpellId(3)),
            equip_right: Some(SpellId(7)),
            mc2_select: Some((1, 2, 3)),
            spell_ring: Some((4, 5)),
            full_stop: true,
            respawn: true,
            demolish: true,
            suicide: true,
            barrel_roll: true,
            raw_dx: -9,
            mc1_move_byte: Some(0x2A),
            mc2_cmd_speed: Some(-1234),
            mc2_park: true,
            cheat: Some(mgc_formats::recover::Cheat::SpellXp),
        };
        let got = flight_input_from(&port_input_from(&want));
        assert_eq!(got.cheat, want.cheat, "retail cheat lane");
        assert_eq!(got.mc2_cmd_speed, want.mc2_cmd_speed, "MC2 cruise lane");
        assert_eq!(got.mc2_park, want.mc2_park, "MC2 park lane");
        assert_eq!(got.suicide, want.suicide, "Shift+K");
        assert_eq!(got.mc1_move_byte, want.mc1_move_byte);
        assert_eq!(got.mc2_select, want.mc2_select);
        assert_eq!(got.spell_ring, want.spell_ring);
        assert_eq!(
            (got.equip_left.map(|s| s.0), got.equip_right.map(|s| s.0)),
            (Some(3), Some(7))
        );
        assert_eq!(
            (
                got.thrust,
                got.strafe,
                got.lift,
                got.yaw_delta,
                got.pitch_delta,
                got.raw_dx
            ),
            (0.25, -0.5, 0.75, 1.5, -2.5, -9)
        );
        assert_eq!((got.stick_x, got.stick_y), (11, -22));
        assert!(
            got.fire_left
                && got.fire_right
                && got.full_stop
                && got.respawn
                && got.demolish
                && got.barrel_roll
        );
    }

    /// **A TAKE CARRIES THE OFFLINE CHASSIS OVERRIDES IT WAS RECORDED
    /// WITH.**
    ///
    /// `--pool-slots` / `--awake-range` are decided before the world is
    /// built, and the take's `start_mgcs_b64` pins them:
    /// `Gen::snap_check_identity` opens on `chassis.pool_slots` and
    /// `chassis.awake_gate_sq`. They were absent from the header and
    /// the replay boot cleared them outright, so a take recorded with
    /// either flag could not be replayed by ANY invocation — not even
    /// the one that recorded it — dying on "snapshot is for a different
    /// world (chassis.pool_slots differs)" long after the point where
    /// the value could still have been used.
    ///
    /// The faithful case must keep reading `None`: that is what every
    /// pre-key take reads as, and the key is omitted entirely rather
    /// than written null, so a default take's header shape is
    /// unchanged.
    #[test]
    fn port_header_carries_the_offline_chassis_params() {
        let mk = |name: &str, pool: Option<usize>, awake: Option<u32>| {
            let path = std::env::temp_dir().join(name);
            let a = Simulation::with_terrain(Vec::new());
            let rec = PortRecorder::begin(
                &path,
                &a,
                "mc1",
                0,
                a.thrust_model,
                a.altitude_model,
                pool,
                awake,
                ImportPin::default(),
            )
            .unwrap();
            rec.finish().2.unwrap();
            let f = ReplayFile::open(&path).unwrap();
            let got = (f.sim_pool_slots, f.sim_awake_range);
            let _ = std::fs::remove_file(&path);
            got
        };
        assert_eq!(
            mk("mgcapp-chassis-set.mgcr", Some(4000), Some(0)),
            (Some(4000), Some(0)),
            "the overrides must survive the round trip"
        );
        // `Some(0)` is a REAL awake_range (always awake), so it must not
        // collapse into the same reading as "not overridden".
        assert_eq!(
            mk("mgcapp-chassis-default.mgcr", None, None),
            (None, None),
            "a faithful take pins nothing"
        );
    }
}

