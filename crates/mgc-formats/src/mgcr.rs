//! The `.mgcr` gameplay-recording format — reader side.
//!
//! `docs/RECORDING.md` is the normative spec (same-commit lockstep
//! clause applies to this module). A recording is a zstd-compressed
//! stream of JSON lines: line 1 = the header record, every following
//! line one tick record in strictly increasing tick order. Retail
//! recordings carry the raw master-struct image per tick (`state`)
//! plus its decoded observable projection (`obs`); this module
//! re-implements the recorder's decode (`tools/mc_dosbox_recorder.py`)
//! so the projection can be re-derived — and byte-checked — from the
//! raw image, and so the conformance runner can type the full retail
//! closure for import.
//!
//! The obs types here serialize to EXACTLY the recorder's JSON (same
//! keys, same integer/float/null choices): `serde_json::to_value` of a
//! decoded [`Obs`] must equal the stored channel value-for-value. The
//! `mgc-conform check-decode` mode pins that equivalence across the
//! recorded corpus.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};

/// MC1/HW master struct `str_AE400_AE3F0` image size (one retail
/// in-level save `fwrite`, docs/traces/mc1-campaign-save-menu.md).
pub const MC1_STRUCT_SIZE: usize = 232_713;
/// MC2 master struct `D41A0_0` image size (in-memory; the SLEV save
/// writes one more byte).
pub const MC2_STRUCT_SIZE: usize = 224_790;

/// Game family: MC1 and Hidden Worlds share one layout; MC2 differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Mc1,
    Mc2,
}

// ---------------------------------------------------------------- container

/// The line-1 header record. Permissive on the free-form provenance
/// fields — only the routing fields are typed.
#[derive(Debug, Clone, Deserialize)]
pub struct Header {
    pub format: u32,
    /// "mc1" | "mc1hw" | "mc2".
    pub game: String,
    #[serde(default)]
    pub level: Option<u32>,
    /// "retail" | "port".
    pub source: String,
    pub tick_hz: u32,
    pub channels: Channels,
    /// Retail address half "A" (CARPET.EXE) / "B" (HIDDEN.EXE); absent
    /// on MC2.
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub tool: Option<serde_json::Value>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub capture: Option<serde_json::Value>,
    /// Port recordings: the pinned sim-config closure.
    #[serde(default)]
    pub sim: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Channels {
    /// "exact" (port) | "raw" (retail externals) | "none".
    pub input: String,
    pub obs: bool,
    pub state: bool,
    pub hash: bool,
}

impl Header {
    pub fn family(&self) -> Result<Family, String> {
        match self.game.as_str() {
            "mc1" | "mc1hw" => Ok(Family::Mc1),
            "mc2" => Ok(Family::Mc2),
            other => Err(format!("unknown game family {other:?}")),
        }
    }

    /// The raw `state.struct_b64` image size this family carries.
    pub fn struct_size(&self) -> Result<usize, String> {
        Ok(match self.family()? {
            Family::Mc1 => MC1_STRUCT_SIZE,
            Family::Mc2 => MC2_STRUCT_SIZE,
        })
    }
}

/// One tick record, container-level view: `obs`/`input` stay untyped
/// JSON here (the strict comparator wants values, not structs); the
/// base64 channels are decoded to bytes.
#[derive(Debug, Clone)]
pub struct TickRecord {
    pub t: u64,
    pub obs: Option<serde_json::Value>,
    /// The raw master-struct image (`state.struct_b64`).
    pub state: Option<Vec<u8>>,
    /// MC1/HW external input registers (`state.ext`), raw bytes.
    pub ext: Option<Ext>,
    pub input: Option<serde_json::Value>,
    pub wallclock: Option<u64>,
}

/// The MC1/HW static-frame input registers (same ±1-tick attribution
/// caveat as the `input` channel — see RECORDING.md).
#[derive(Debug, Clone, Default)]
pub struct Ext {
    /// 128 pressed-scancode cells (0 = up).
    pub keys: Option<Vec<u8>>,
    /// Mouse cursor {i16 x, i16 y}.
    pub cursor: Option<(i16, i16)>,
    /// Held mouse buttons (nonzero i16 = held).
    pub lbtn: Option<i16>,
    pub rbtn: Option<i16>,
}

fn b64_field(v: &serde_json::Value, key: &str) -> Result<Option<Vec<u8>>, String> {
    use base64::Engine as _;
    match v.get(key) {
        None => Ok(None),
        Some(s) => {
            let s = s.as_str().ok_or_else(|| format!("{key}: not a string"))?;
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map(Some)
                .map_err(|e| format!("{key}: bad base64: {e}"))
        }
    }
}

impl TickRecord {
    pub fn from_value(v: &serde_json::Value) -> Result<TickRecord, String> {
        let t = v
            .get("t")
            .and_then(|t| t.as_u64())
            .ok_or("tick record without a numeric \"t\"")?;
        let (state, ext) = match v.get("state") {
            None => (None, None),
            Some(st) => {
                let image = b64_field(st, "struct_b64")?;
                let ext = match st.get("ext") {
                    None => None,
                    Some(e) => {
                        let cursor = b64_field(e, "cursor_b64")?.map(|b| {
                            (
                                i16::from_le_bytes([b[0], b[1]]),
                                i16::from_le_bytes([b[2], b[3]]),
                            )
                        });
                        let btn = |b: Option<Vec<u8>>| b.map(|b| i16::from_le_bytes([b[0], b[1]]));
                        Some(Ext {
                            keys: b64_field(e, "keys_b64")?,
                            cursor,
                            lbtn: btn(b64_field(e, "lbtn_b64")?),
                            rbtn: btn(b64_field(e, "rbtn_b64")?),
                        })
                    }
                };
                (image, ext)
            }
        };
        Ok(TickRecord {
            t,
            obs: v.get("obs").cloned(),
            state,
            ext,
            input: v.get("input").cloned(),
            wallclock: v.get("wallclock").and_then(|w| w.as_u64()),
        })
    }
}

/// A `.mgcr` (zstd) or plain `.jsonl` recording, opened for streaming.
/// Compression is sniffed from the zstd magic, not the extension.
pub struct Recording {
    pub header: Header,
    lines: std::io::Lines<Box<dyn BufRead>>,
    pub line_no: u64,
}

impl Recording {
    pub fn open(path: &std::path::Path) -> Result<Recording, String> {
        let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut magic = [0u8; 4];
        let got = {
            use std::io::Read as _;
            let n = f.read(&mut magic).map_err(|e| e.to_string())?;
            use std::io::Seek as _;
            f.seek(std::io::SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            n
        };
        let zstd_magic = got == 4 && magic == [0x28, 0xB5, 0x2F, 0xFD];
        let reader: Box<dyn BufRead> = if zstd_magic {
            let dec: zstd::stream::read::Decoder<'_, BufReader<std::fs::File>> =
                zstd::stream::read::Decoder::new(f).map_err(|e| e.to_string())?;
            Box::new(BufReader::new(dec))
        } else {
            Box::new(BufReader::new(f))
        };
        let mut lines = BufRead::lines(reader);
        let first = lines
            .next()
            .ok_or("empty recording")?
            .map_err(|e| e.to_string())?;
        let header: Header =
            serde_json::from_str(&first).map_err(|e| format!("header parse: {e}"))?;
        if header.format != 1 {
            return Err(format!("unsupported .mgcr format {}", header.format));
        }
        Ok(Recording {
            header,
            lines,
            line_no: 1,
        })
    }

    /// Next tick record as raw JSON, or None at end of stream.
    pub fn next_value(&mut self) -> Option<Result<serde_json::Value, String>> {
        let line = match self.lines.next()? {
            Ok(l) => l,
            Err(e) => return Some(Err(e.to_string())),
        };
        self.line_no += 1;
        if line.trim().is_empty() {
            return self.next_value();
        }
        Some(serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", self.line_no)))
    }

    /// Next tick record, typed at the container level.
    pub fn next_tick(&mut self) -> Option<Result<TickRecord, String>> {
        let v = match self.next_value()? {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        Some(TickRecord::from_value(&v))
    }
}

// ------------------------------------------------------------- byte readers

fn u8_(d: &[u8], o: usize) -> u8 {
    d[o]
}
fn i8_(d: &[u8], o: usize) -> i8 {
    d[o] as i8
}
fn u16_(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn i16_(d: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([d[o], d[o + 1]])
}
fn u32_(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
fn i32_(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

// -------------------------------------------------------- obs schema: MC1/HW

/// MC1/HW struct geometry (`str_AE400_AE3F0`; offsets are the
/// recorder's `LAYOUT_MC1`, themselves the Basic.h field names).
mod m1 {
    pub const RNG: usize = 4;
    pub const LOCAL_PLAYER: usize = 8; // +2 = player count
    pub const WIZARDS: usize = 13_323;
    pub const WIZARD_STRIDE: usize = 2_049;
    pub const WIZARD_COUNT: usize = 8;
    pub const WIZ_PLAYINDEX: usize = 10;
    pub const T160: usize = 1_103; // Type_160 within the wizard record
    pub const T160_HAND_L: usize = 940;
    pub const T160_HAND_R: usize = 944;
    pub const CTRL: usize = 29_715;
    pub const CTRL_STRIDE: usize = 10;
    pub const CTRL_COUNT: usize = 8;
    pub const POOL: usize = 29_795;
    pub const ENT_STRIDE: usize = 164;
    pub const ENT_COUNT: usize = 1_000;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObsMc1 {
    pub rng: u32,
    pub n_active: u32,
    pub local_player: u16,
    pub player_count: u16,
    pub wizards: Vec<WizardMc1>,
    pub control: Vec<ControlMc1>,
    pub player: Option<PlayerJoinMc1>,
    pub entities: Vec<EntObsMc1>,
}

/// One active MC1 pool entity, the recorder's projection (slot 0 and
/// class-0 slots are never emitted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntObsMc1 {
    pub slot: u16,
    pub class: u8,
    pub model: u8,
    pub sclass: u8,
    pub smodel: u8,
    pub flags: u32,
    pub id: u16,
    pub life: i32,
    pub max_life: u32,
    /// 8.8 fixed → exact float (tile units).
    pub x: f64,
    pub y: f64,
    pub z: i16,
    /// Applied yaw, 11-bit engine angle.
    pub heading: u16,
    pub pitch: u16,
    pub target_yaw: u16,
    pub speed: i16,
    pub mana: u32,
    pub mana_max: u32,
    pub chase: u16,
    /// Raw guest pointer to the owner wizard's Type_160.
    pub owner_ptr: u32,
    pub tick_byte: u8,
    pub rand: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WizardMc1 {
    pub index: u16,
    pub play_index: u16,
    pub hand_left: Option<u16>,
    pub hand_right: Option<u16>,
    pub castle: u16,
    pub flight: FlightMc1,
}

/// Persistent flight/steering accumulators (survive the tick, unlike
/// the consumed control slot).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightMc1 {
    pub cmd_speed: i16,
    pub strafe: i16,
    pub roll_acc: u16,
    pub pitch_acc: u16,
}

/// The 10-byte per-player control command (reads all-zero at a
/// between-tick snapshot — consumed mid-tick).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlMc1 {
    pub player: u16,
    pub opcode: u8,
    pub param1: u8,
    pub param2: u8,
    pub aim_yaw: i8,
    pub aim_pitch: i8,
    pub move_fire: u8,
    pub thrust: bool,
    pub decel: bool,
    pub strafe_left: bool,
    pub strafe_right: bool,
    pub fire_left: bool,
    pub fire_right: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerJoinMc1 {
    pub carpet_slot: u16,
    pub life: i32,
    pub max_life: u32,
    pub mana: u32,
    pub mana_max: u32,
    pub x: f64,
    pub y: f64,
    pub z: i16,
    pub heading: u16,
    pub pitch: u16,
    pub speed: i16,
    pub hand_left: Option<u16>,
    pub hand_right: Option<u16>,
    pub castle: u16,
    pub flight: FlightMc1,
    pub control: Option<ControlMc1>,
}

fn hand_mc1(v: u16) -> Option<u16> {
    if v == 0xFFFF || v == 0xFF {
        None
    } else {
        Some(v)
    }
}

fn ent_obs_mc1(d: &[u8], slot: u16) -> EntObsMc1 {
    let o = m1::POOL + slot as usize * m1::ENT_STRIDE;
    EntObsMc1 {
        slot,
        class: u8_(d, o + 64),
        model: u8_(d, o + 65),
        sclass: u8_(d, o + 66),
        smodel: u8_(d, o + 67),
        flags: u32_(d, o + 16),
        id: u16_(d, o + 24),
        life: i32_(d, o + 12),
        max_life: u32_(d, o + 8),
        x: u16_(d, o + 72) as f64 / 256.0,
        y: u16_(d, o + 74) as f64 / 256.0,
        z: i16_(d, o + 76),
        heading: u16_(d, o + 30),
        pitch: u16_(d, o + 32),
        target_yaw: u16_(d, o + 34),
        speed: i16_(d, o + 126),
        mana: u32_(d, o + 140),
        mana_max: u32_(d, o + 136),
        chase: u16_(d, o + 146),
        owner_ptr: u32_(d, o + 160),
        tick_byte: u8_(d, o + 63),
        rand: u32_(d, o + 4),
    }
}

fn wizard_mc1(d: &[u8], i: u16) -> WizardMc1 {
    let w = m1::WIZARDS + i as usize * m1::WIZARD_STRIDE;
    let t = w + m1::T160;
    WizardMc1 {
        index: i,
        play_index: u16_(d, w + m1::WIZ_PLAYINDEX),
        hand_left: hand_mc1(u16_(d, t + m1::T160_HAND_L)),
        hand_right: hand_mc1(u16_(d, t + m1::T160_HAND_R)),
        castle: u16_(d, t + 50),
        flight: FlightMc1 {
            cmd_speed: i16_(d, t + 12),
            strafe: i16_(d, t + 16),
            roll_acc: u16_(d, t + 327),
            pitch_acc: u16_(d, t + 329),
        },
    }
}

fn control_mc1(d: &[u8], player: u16) -> ControlMc1 {
    let o = m1::CTRL + player as usize * m1::CTRL_STRIDE;
    let mv = u8_(d, o + 5);
    ControlMc1 {
        player,
        opcode: u8_(d, o),
        param1: u8_(d, o + 1),
        param2: u8_(d, o + 2),
        aim_yaw: i8_(d, o + 3),
        aim_pitch: i8_(d, o + 4),
        move_fire: mv,
        thrust: mv & 1 != 0,
        decel: mv & 2 != 0,
        strafe_left: mv & 4 != 0,
        strafe_right: mv & 8 != 0,
        fire_left: mv & 16 != 0,
        fire_right: mv & 32 != 0,
    }
}

/// Decode the MC1/HW obs projection from a raw struct image — the
/// recorder's `decode_snapshot`, key for key.
pub fn decode_obs_mc1(d: &[u8]) -> ObsMc1 {
    let local = u16_(d, m1::LOCAL_PLAYER);
    let pcount = u16_(d, m1::LOCAL_PLAYER + 2);
    let entities: Vec<EntObsMc1> = (1..m1::ENT_COUNT as u16)
        .filter(|&s| d[m1::POOL + s as usize * m1::ENT_STRIDE + 64] != 0)
        .map(|s| ent_obs_mc1(d, s))
        .collect();
    let wizards: Vec<WizardMc1> = (0..m1::WIZARD_COUNT as u16)
        .map(|i| wizard_mc1(d, i))
        .collect();
    let control: Vec<ControlMc1> = (0..m1::CTRL_COUNT as u16)
        .map(|p| control_mc1(d, p))
        .collect();
    let player = wizards.get(local as usize).and_then(|w| {
        let carpet = entities.iter().find(|e| e.slot == w.play_index)?;
        Some(PlayerJoinMc1 {
            carpet_slot: w.play_index,
            life: carpet.life,
            max_life: carpet.max_life,
            mana: carpet.mana,
            mana_max: carpet.mana_max,
            x: carpet.x,
            y: carpet.y,
            z: carpet.z,
            heading: carpet.heading,
            pitch: carpet.pitch,
            speed: carpet.speed,
            hand_left: w.hand_left,
            hand_right: w.hand_right,
            castle: w.castle,
            flight: w.flight.clone(),
            control: control.get(local as usize).cloned(),
        })
    });
    ObsMc1 {
        rng: u32_(d, m1::RNG),
        n_active: entities.len() as u32,
        local_player: local,
        player_count: pcount,
        wizards,
        control,
        player,
        entities,
    }
}

// --------------------------------------------------------- obs schema: MC2

/// MC2 struct geometry (`D41A0_0`; the recorder's `LAYOUT_MC2`, from
/// the remc2 `LevelStructs.h` pass — see the mc2 recorder field map).
mod m2 {
    pub const RNG: usize = 8;
    pub const LOCAL_PLAYER: usize = 0xC; // +2 = player count
    pub const PLAYERS: usize = 0x2BDE;
    pub const PLAYER_STRIDE: usize = 2_124;
    pub const PP_ISAI: usize = 0x9;
    pub const PP_PLAYINDEX: usize = 0xA;
    pub const PP_TURN: usize = 0x12;
    pub const PP_NAME: usize = 0x39F;
    pub const PP_CASTLE: usize = 1_080;
    pub const PP_HAND_L: usize = 2_103;
    pub const PP_HAND_R: usize = 2_105;
    pub const PP_FLIGHT: usize = 998; // type_str_164
    pub const CTRL: usize = 0x6E3E;
    pub const CTRL_STRIDE: usize = 10;
    pub const POOL: usize = 0x6E8E;
    pub const ENT_STRIDE: usize = 168;
    pub const ENT_COUNT: usize = 1_000;
    pub const ENT_CLASS: usize = 0x3F;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObsMc2 {
    pub rng: u32,
    pub n_active: u32,
    pub local_player: u16,
    pub player_count: u16,
    pub players: Vec<PlayerMc2>,
    pub control: Vec<ControlMc2>,
    pub player: Option<PlayerJoinMc2>,
    pub entities: Vec<EntObsMc2>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntObsMc2 {
    pub slot: u16,
    pub class: u8,
    pub model: u8,
    pub life: i32,
    pub max_life: i32,
    pub x: f64,
    pub y: f64,
    pub z: i16,
    /// World yaw — the live facing (the applied yaw @0x52 rests at a
    /// constant for the player; captured separately).
    pub heading: i16,
    pub pitch: i16,
    pub applied_yaw: i16,
    pub applied_pitch: i16,
    pub speed: i16,
    pub mana: i32,
    pub mana_max: i32,
    /// parentId @0x28.
    pub owner: u16,
    pub action: u8,
    pub sv1: u8,
    pub sv2: u8,
    pub player_ent_idx: u16,
    pub rand: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerMc2 {
    pub index: u16,
    pub is_ai: bool,
    pub play_index: u16,
    pub turn: i32,
    pub name: String,
    pub castle: i16,
    pub hand_left: Option<i16>,
    pub hand_right: Option<i16>,
    pub flight: FlightMc2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightMc2 {
    pub cmd_speed: i16,
    pub v16: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlMc2 {
    pub player: u16,
    pub opcode: u8,
    pub param1: u8,
    pub param2: u8,
    pub aim_yaw: i8,
    pub aim_pitch: i8,
    pub buttons: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerJoinMc2 {
    pub carpet_slot: u16,
    pub name: String,
    pub is_ai: bool,
    pub turn: i32,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub mana_max: i32,
    pub x: f64,
    pub y: f64,
    pub z: i16,
    pub heading: i16,
    pub pitch: i16,
    pub applied_yaw: i16,
    pub applied_pitch: i16,
    pub speed: i16,
    pub hand_left: Option<i16>,
    pub hand_right: Option<i16>,
    pub castle: i16,
    pub flight: FlightMc2,
    pub control: Option<ControlMc2>,
}

fn hand_mc2(v: i16) -> Option<i16> {
    if v < 0 { None } else { Some(v) }
}

/// Latin-1, NUL-terminated, capped at 24 — the recorder's `_name`.
fn name_mc2(d: &[u8], o: usize) -> String {
    d[o..o + 24]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect()
}

fn ent_obs_mc2(d: &[u8], slot: u16) -> EntObsMc2 {
    let o = m2::POOL + slot as usize * m2::ENT_STRIDE;
    EntObsMc2 {
        slot,
        class: u8_(d, o + 0x3F),
        model: u8_(d, o + 0x40),
        life: i32_(d, o + 0x08),
        max_life: i32_(d, o + 0x04),
        x: u16_(d, o + 0x4C) as f64 / 256.0,
        y: u16_(d, o + 0x4E) as f64 / 256.0,
        z: i16_(d, o + 0x50),
        heading: i16_(d, o + 0x1C),
        pitch: i16_(d, o + 0x1E),
        applied_yaw: i16_(d, o + 0x52),
        applied_pitch: i16_(d, o + 0x54),
        speed: i16_(d, o + 0x82),
        mana: i32_(d, o + 0x90),
        mana_max: i32_(d, o + 0x8C),
        owner: u16_(d, o + 0x28),
        action: u8_(d, o + 0x45),
        sv1: u8_(d, o + 0x48),
        sv2: u8_(d, o + 0x49),
        player_ent_idx: u16_(d, o + 0x94),
        rand: u16_(d, o + 0x14),
    }
}

fn player_mc2(d: &[u8], i: u16) -> PlayerMc2 {
    let b = m2::PLAYERS + i as usize * m2::PLAYER_STRIDE;
    let t = b + m2::PP_FLIGHT;
    PlayerMc2 {
        index: i,
        is_ai: u8_(d, b + m2::PP_ISAI) != 0,
        play_index: u16_(d, b + m2::PP_PLAYINDEX),
        turn: i32_(d, b + m2::PP_TURN),
        name: name_mc2(d, b + m2::PP_NAME),
        castle: i16_(d, b + m2::PP_CASTLE),
        hand_left: hand_mc2(i16_(d, b + m2::PP_HAND_L)),
        hand_right: hand_mc2(i16_(d, b + m2::PP_HAND_R)),
        flight: FlightMc2 {
            cmd_speed: i16_(d, t + 12),
            v16: i16_(d, t + 16),
        },
    }
}

fn control_mc2(d: &[u8], player: u16) -> ControlMc2 {
    let o = m2::CTRL + player as usize * m2::CTRL_STRIDE;
    ControlMc2 {
        player,
        opcode: u8_(d, o),
        param1: u8_(d, o + 1),
        param2: u8_(d, o + 2),
        aim_yaw: i8_(d, o + 3),
        aim_pitch: i8_(d, o + 4),
        buttons: u8_(d, o + 5),
    }
}

/// Decode the MC2 obs projection from a raw struct image.
pub fn decode_obs_mc2(d: &[u8]) -> ObsMc2 {
    let local = u16_(d, m2::LOCAL_PLAYER);
    let pcount = u16_(d, m2::LOCAL_PLAYER + 2);
    let entities: Vec<EntObsMc2> = (1..m2::ENT_COUNT as u16)
        .filter(|&s| d[m2::POOL + s as usize * m2::ENT_STRIDE + m2::ENT_CLASS] != 0)
        .map(|s| ent_obs_mc2(d, s))
        .collect();
    let players: Vec<PlayerMc2> = (0..pcount).map(|p| player_mc2(d, p)).collect();
    let control: Vec<ControlMc2> = (0..pcount).map(|p| control_mc2(d, p)).collect();
    let player = players.get(local as usize).and_then(|p| {
        let carpet = entities.iter().find(|e| e.slot == p.play_index)?;
        Some(PlayerJoinMc2 {
            carpet_slot: p.play_index,
            name: p.name.clone(),
            is_ai: p.is_ai,
            turn: p.turn,
            life: carpet.life,
            max_life: carpet.max_life,
            mana: carpet.mana,
            mana_max: carpet.mana_max,
            x: carpet.x,
            y: carpet.y,
            z: carpet.z,
            heading: carpet.heading,
            pitch: carpet.pitch,
            applied_yaw: carpet.applied_yaw,
            applied_pitch: carpet.applied_pitch,
            speed: carpet.speed,
            hand_left: p.hand_left,
            hand_right: p.hand_right,
            castle: p.castle,
            flight: p.flight.clone(),
            control: control.get(local as usize).cloned(),
        })
    });
    ObsMc2 {
        rng: u32_(d, m2::RNG),
        n_active: entities.len() as u32,
        local_player: local,
        player_count: pcount,
        players,
        control,
        player,
        entities,
    }
}

// ----------------------------------------------------------- obs dispatch

/// A decoded obs projection, either family.
#[derive(Debug, Clone, PartialEq)]
pub enum Obs {
    Mc1(ObsMc1),
    Mc2(ObsMc2),
}

impl Obs {
    /// Decode from a raw master-struct image.
    pub fn decode(family: Family, image: &[u8]) -> Result<Obs, String> {
        match family {
            Family::Mc1 => {
                if image.len() != MC1_STRUCT_SIZE {
                    return Err(format!(
                        "MC1 struct image is {} bytes, want {MC1_STRUCT_SIZE}",
                        image.len()
                    ));
                }
                Ok(Obs::Mc1(decode_obs_mc1(image)))
            }
            Family::Mc2 => {
                if image.len() != MC2_STRUCT_SIZE {
                    return Err(format!(
                        "MC2 struct image is {} bytes, want {MC2_STRUCT_SIZE}",
                        image.len()
                    ));
                }
                Ok(Obs::Mc2(decode_obs_mc2(image)))
            }
        }
    }

    /// The recorder-schema JSON value (what the obs channel stores).
    pub fn to_value(&self) -> serde_json::Value {
        match self {
            Obs::Mc1(o) => serde_json::to_value(o).expect("obs serialize"),
            Obs::Mc2(o) => serde_json::to_value(o).expect("obs serialize"),
        }
    }

    /// Parse a stored obs channel value.
    pub fn from_value(family: Family, v: &serde_json::Value) -> Result<Obs, String> {
        match family {
            Family::Mc1 => serde_json::from_value(v.clone())
                .map(Obs::Mc1)
                .map_err(|e| format!("obs parse (mc1): {e}")),
            Family::Mc2 => serde_json::from_value(v.clone())
                .map(Obs::Mc2)
                .map_err(|e| format!("obs parse (mc2): {e}")),
        }
    }
}

// --------------------------------------- full retail closure, MC1 (import)

/// One fully-decoded MC1 pool entity — every `Type_AE400_29795` field
/// the port's `Ent` models, plus the two guest pointers (stable guest
/// addresses; the conformance importer converts them to indices).
/// Offsets per Basic.h:368-442.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetailEntMc1 {
    pub rand: u32,     // +4
    pub max_life: u32, // +8
    pub act_life: i32, // +12
    pub flags: u32,    // +16
    pub next20: u16,   // +20 (tile-list link, rebuilt per tick)
    pub prev22: u16,   // +22
    pub id24: u16,     // +24
    pub f26: i16,      // +26
    pub f28: u16,      // +28
    pub f30: u16,      // +30
    pub f32: u16,      // +32
    pub f34: u16,      // +34
    pub f36: u16,      // +36
    pub f38: u16,      // +38
    pub f40: u16,      // +40
    pub f42: u16,      // +42
    pub f44: u16,      // +44
    pub f46: i16,      // +46
    pub f48: u16,      // +48
    pub f50: i16,      // +50
    pub f52: u16,      // +52
    pub f54: u16,      // +54
    pub f56: u16,      // +56
    pub f58: i8,       // +58 (one byte in retail; the port widens)
    pub f59: u8,       // +59
    pub f61: u8,       // +61
    pub f62: u8,       // +62
    pub f63: u8,       // +63 (the per-tick continuity byte)
    pub class64: u8,   // +64 (0 = free slot)
    pub model65: u8,   // +65
    pub f66: u8,       // +66 (sClass / team)
    pub f67: u8,       // +67 (sModel)
    pub f68: u8,       // +68 (explosion class)
    pub f69: u8,       // +69 (explosion model)
    pub f70: u8,       // +70 (tick-handler index)
    pub f71: u8,       // +71
    pub x: u16,        // +72 (8.8)
    pub y: u16,        // +74
    pub z: i16,        // +76
    pub f78: u16,      // +78
    pub f80: u16,      // +80
    pub f82: u16,      // +82
    pub f84: u16,      // +84
    pub type86: u16,   // +86
    pub frame88: u8,   // +88
    pub frames89: u8,  // +89
    /// Damage mailboxes +90..126: six {u32 amount, u16 source}.
    pub mail: [(u32, u16); 6],
    pub f126: i16,   // +126 (actual speed)
    pub f128: i16,   // +128 (target speed)
    pub f130: i16,   // +130 (acceleration)
    pub f132: u16,   // +132
    pub f136: i32,   // +136 (mana cap)
    pub f140: i32,   // +140 (mana)
    pub f144: u16,   // +144 (ball owner)
    pub f146: u16,   // +146 (chase target slot)
    pub dest_x: u16, // +150
    pub dest_y: u16, // +152
    pub site_z: i16, // +154
    /// Guest pointer `Type_156*` @156 — behavior row = (ptr − 0x98F38)/32.
    pub model_ptr: u32,
    /// Guest pointer `Type_160*` @160 — the owner wizard's spell column.
    pub owner_ptr: u32,
}

/// One decoded MC1 wizard record (`TypeStrAE400_13323`, 2049 B) — the
/// slice of the Type_160 spell/flight column the importer consumes.
#[derive(Debug, Clone, Copy)]
pub struct RetailWizardMc1 {
    /// Exit-status word `var_u16_13325` (+2; bit 2 = won).
    pub status: u16,
    /// Pool slot of this wizard's carpet (+10).
    pub play_index: u16,
    // Type_160 (wizard+1103) fields, by their in-record offsets:
    pub cmd_speed: i16, // +12
    pub strafe: i16,    // +16
    /// Knock/buffet channel: magnitude v_22 / direction v_24.
    pub knock_mag: i16, // +22
    pub knock_dir: u16, // +24
    /// Danger-music countdown (v_46).
    pub danger: i16, // +46
    pub castle: u16,    // +50 (established castle slot)
    /// Claimed-house mana tally (u32_308).
    pub banked_houses: i32, // +308
    pub roll_acc: u16,  // +327
    pub pitch_acc: u16, // +329
    /// Spawn grace (u16_331): mailbox wiped while > 0.
    pub grace: u16,
    pub shots: u32, // +343
    pub hits: u32,  // +347
    pub kills: u32, // +359
    /// Village-aggro timer (+528).
    pub aggro: i16,
    /// The owned-spell ACQUISITION list (+532, i32[24]): while alive,
    /// manifestation pool slots in pickup order (the death path
    /// rewrites it to model numbers for the respawn re-grant). The
    /// hand indices @940/@944 index into THIS list.
    pub spell_list: [i32; 24],
    /// Owned-spell manifestation pool slots BY INTERNAL SPELL ID
    /// (var_676, u16[24], 0 = not owned).
    pub owned_slots: [u16; 24],
    /// Blue-grant flags (var_916, u8[24]).
    pub blue: [u8; 24],
    /// RAW hand indices (+940/+944): index into `spell_list`, 255 /
    /// 0xFFFF = empty hand. Resolve to a spell id via
    /// [`RetailMc1::hand_spell`].
    pub hand_left: u16, // +940
    pub hand_right: u16, // +944
    /// HUD alert countdowns: castle/self/balloon under attack.
    pub castle_alert: u8, // +391
    pub player_alert: u8, // +392
    pub balloon_alert: u8, // +393
}

/// The typed MC1 retail closure the conformance importer consumes.
#[derive(Debug, Clone)]
pub struct RetailMc1 {
    pub rand: u32,
    pub local_player: u16,
    pub player_count: u16,
    /// Per-model spawn ordinals (`str_12`, struct +12..32).
    pub spawn_count: [u8; 20],
    pub wizards: Vec<RetailWizardMc1>,
    /// All 1000 pool records, indexed by slot (slot 0 included).
    pub ents: Vec<RetailEntMc1>,
    /// The LIVE free-slot stack (+40 top index, +593 pointer cells;
    /// guest pointers are stable so cells convert to slots). Retail
    /// pops `entries[top]` then decrements — the LAST element here is
    /// the next allocation. Retail's own load discards and rebuilds
    /// this by slot scan, but the live order is what makes port-side
    /// spawns land on the same slots the recording's do.
    pub free_stack: Vec<u16>,
    /// The recycle stack (+4593 top, −1 = empty; +4597 cells).
    pub recycle_stack: Vec<u16>,
    /// Level number stashed in the tail (+232707).
    pub level: u16,
}

pub fn decode_retail_ent_mc1(d: &[u8], slot: u16) -> RetailEntMc1 {
    let o = m1::POOL + slot as usize * m1::ENT_STRIDE;
    let mut mail = [(0u32, 0u16); 6];
    for (ch, m) in mail.iter_mut().enumerate() {
        *m = (u32_(d, o + 90 + ch * 6), u16_(d, o + 94 + ch * 6));
    }
    RetailEntMc1 {
        rand: u32_(d, o + 4),
        max_life: u32_(d, o + 8),
        act_life: i32_(d, o + 12),
        flags: u32_(d, o + 16),
        next20: u16_(d, o + 20),
        prev22: u16_(d, o + 22),
        id24: u16_(d, o + 24),
        f26: i16_(d, o + 26),
        f28: u16_(d, o + 28),
        f30: u16_(d, o + 30),
        f32: u16_(d, o + 32),
        f34: u16_(d, o + 34),
        f36: u16_(d, o + 36),
        f38: u16_(d, o + 38),
        f40: u16_(d, o + 40),
        f42: u16_(d, o + 42),
        f44: u16_(d, o + 44),
        f46: i16_(d, o + 46),
        f48: u16_(d, o + 48),
        f50: i16_(d, o + 50),
        f52: u16_(d, o + 52),
        f54: u16_(d, o + 54),
        f56: u16_(d, o + 56),
        f58: i8_(d, o + 58),
        f59: u8_(d, o + 59),
        f61: u8_(d, o + 61),
        f62: u8_(d, o + 62),
        f63: u8_(d, o + 63),
        class64: u8_(d, o + 64),
        model65: u8_(d, o + 65),
        f66: u8_(d, o + 66),
        f67: u8_(d, o + 67),
        f68: u8_(d, o + 68),
        f69: u8_(d, o + 69),
        f70: u8_(d, o + 70),
        f71: u8_(d, o + 71),
        x: u16_(d, o + 72),
        y: u16_(d, o + 74),
        z: i16_(d, o + 76),
        f78: u16_(d, o + 78),
        f80: u16_(d, o + 80),
        f82: u16_(d, o + 82),
        f84: u16_(d, o + 84),
        type86: u16_(d, o + 86),
        frame88: u8_(d, o + 88),
        frames89: u8_(d, o + 89),
        mail,
        f126: i16_(d, o + 126),
        f128: i16_(d, o + 128),
        f130: i16_(d, o + 130),
        f132: u16_(d, o + 132),
        f136: i32_(d, o + 136),
        f140: i32_(d, o + 140),
        f144: u16_(d, o + 144),
        f146: u16_(d, o + 146),
        dest_x: u16_(d, o + 150),
        dest_y: u16_(d, o + 152),
        site_z: i16_(d, o + 154),
        model_ptr: u32_(d, o + 156),
        owner_ptr: u32_(d, o + 160),
    }
}

fn decode_retail_wizard_mc1(d: &[u8], i: u16) -> RetailWizardMc1 {
    let w = m1::WIZARDS + i as usize * m1::WIZARD_STRIDE;
    let t = w + m1::T160;
    let mut spell_list = [0i32; 24];
    let mut owned_slots = [0u16; 24];
    let mut blue = [0u8; 24];
    for s in 0..24 {
        spell_list[s] = i32_(d, t + 532 + s * 4);
        owned_slots[s] = u16_(d, t + 676 + s * 2);
        blue[s] = u8_(d, t + 916 + s);
    }
    RetailWizardMc1 {
        status: u16_(d, w + 2),
        play_index: u16_(d, w + m1::WIZ_PLAYINDEX),
        cmd_speed: i16_(d, t + 12),
        strafe: i16_(d, t + 16),
        knock_mag: i16_(d, t + 22),
        knock_dir: u16_(d, t + 24),
        danger: i16_(d, t + 46),
        castle: u16_(d, t + 50),
        banked_houses: i32_(d, t + 308),
        roll_acc: u16_(d, t + 327),
        pitch_acc: u16_(d, t + 329),
        grace: u16_(d, t + 331),
        shots: u32_(d, t + 343),
        hits: u32_(d, t + 347),
        kills: u32_(d, t + 359),
        aggro: i16_(d, t + 528),
        spell_list,
        owned_slots,
        blue,
        hand_left: u16_(d, t + m1::T160_HAND_L),
        hand_right: u16_(d, t + m1::T160_HAND_R),
        castle_alert: u8_(d, t + 391),
        player_alert: u8_(d, t + 392),
        balloon_alert: u8_(d, t + 393),
    }
}

impl RetailMc1 {
    /// Resolve a wizard's RAW hand index (+940/+944) to the equipped
    /// INTERNAL spell id: index into the acquisition list → the
    /// manifestation pool slot → its model byte. None = empty hand or
    /// an unresolvable entry.
    pub fn hand_spell(&self, wizard: usize, raw: u16) -> Option<u8> {
        if raw == 0xFFFF || raw == 0xFF {
            return None;
        }
        let slot = *self.wizards.get(wizard)?.spell_list.get(raw as usize)?;
        let e = self.ents.get(usize::try_from(slot).ok()?)?;
        (e.class64 == 12).then_some(e.model65)
    }
}

// --------------------------------------- full retail closure, MC2 (import)

/// One fully-decoded MC2 pool entity (`type_shadow_str_0x6E8E`, 168 B,
/// remc2 LevelStructs.h) — every field, by retail offset. Field names
/// here are RETAIL-offset flavored (hex offsets); the conformance
/// importer translates them onto the port's [`Ent`] via the SEMANTIC
/// alias table (mc2/mobs.rs) — MC2 offsets do NOT line up with the
/// port's MC1-numbered fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetailEntMc2 {
    pub next0: u32,     // +0x00 (list link)
    pub max_life: i32,  // +0x04 (header comment says LIVE; live-dump verified MAX)
    pub life: i32,      // +0x08
    pub flags: u32,     // +0x0C (dw_w_b flag dword)
    pub scratch10: i32, // +0x10 (dword_0x10_16 — scratch/invis)
    pub rand: u16,      // +0x14 (u16 LCG stream)
    pub f16: u16,       // +0x16
    pub next18: u16,    // +0x18 (tile-chain next entity)
    pub f1a: u16,       // +0x1A
    pub yaw: i16,       // +0x1C (world yaw — the live facing)
    pub pitch: i16,     // +0x1E
    pub roll: i16,      // +0x20 (target-yaw channel)
    pub f22: i16,       // +0x22
    pub f24: i16,       // +0x24 (killer)
    pub f26: i16,       // +0x26 (hit source)
    pub owner28: u16,   // +0x28 (parentId — WHO OWNS ME)
    pub f2a: u16,       // +0x2A (subSpellIndex)
    pub f2c: i16,       // +0x2C
    pub f2e: i16,       // +0x2E
    pub f30: u16,       // +0x30
    pub f32: u16,       // +0x32 (pack leader)
    pub f34: u16,       // +0x34 (subentity chain)
    pub f36: u16,       // +0x36
    pub b38: i8,        // +0x38
    pub b39: i8,        // +0x39 (awake; 0xFA dead sentinel)
    pub b3a: i8,        // +0x3A (wake delay)
    pub b3b: i8,        // +0x3B
    pub b3c: i8,        // +0x3C
    pub b3d: i8,        // +0x3D
    pub phase3e: u8,    // +0x3E (per-handler-run phase byte)
    pub class3f: u8,    // +0x3F (0 = free slot)
    pub model40: u8,    // +0x40
    pub b41: i8,        // +0x41 (xtype)
    pub b42: i8,        // +0x42 (xsubtype)
    pub b43: i8,        // +0x43
    pub b44: i8,        // +0x44
    pub action45: u8,   // +0x45 (state/actionIndex)
    pub b46: i8,        // +0x46
    pub b47: i8,        // +0x47
    pub sv1: i8,        // +0x48 (StageVar1)
    pub sv2: i8,        // +0x49 (StageVar2)
    pub sv_timer: i16,  // +0x4A
    pub x: u16,         // +0x4C (8.8)
    pub y: u16,         // +0x4E
    pub z: i16,         // +0x50
    pub ayaw: i16,      // +0x52 (applied yaw)
    pub apitch: i16,    // +0x54
    pub aroll: i16,     // +0x56
    pub afov: i16,      // +0x58
    pub f5a: i16,       // +0x5A (sprite-param index)
    pub b5c: i8,        // +0x5C (anim frame)
    pub b5d: i8,        // +0x5D
    /// Damage mailboxes +0x5E..0x82: six {i32 amount, u16 source}
    /// (type_str_0x5E_94 — same 36-byte 6-channel shape as MC1's).
    pub mail: [(i32, u16); 6],
    pub speed: i16,      // +0x82
    pub min_speed: i16,  // +0x84
    pub max_speed: i16,  // +0x86
    pub d88: i32,        // +0x88 (mana regen)
    pub mana_max: i32,   // +0x8C
    pub mana: i32,       // +0x90
    pub player_ent: u16, // +0x94
    pub target96: u16,   // +0x96
    pub f98: u16,        // +0x98
    pub dest_x: u16,     // +0x9A (axis_0x9A_154x)
    pub dest_y: u16,     // +0x9C
    pub dest_z: i16,     // +0x9E
    /// Guest pointer `dword_0xA0_160x` (special settings / str_160).
    pub ptr_a0: u32,
    /// Guest pointer `dword_0xA4_164x` (str_164).
    pub ptr_a4: u32,
}

/// One decoded MC2 per-player block (`type_str_0x2BDE`, 2124 B) — the
/// slice the conformance importer consumes.
#[derive(Debug, Clone, Copy)]
pub struct RetailPlayerMc2 {
    pub flags: u32,      // +0x00
    pub is_ai: bool,     // +0x09
    pub play_index: u16, // +0x0A (carpet entity slot)
    pub turn: i32,       // +0x12 (per-frame counter; local player only)
    pub castle: i16,     // +1080 (CastleEntityIndex, block-relative)
    pub cmd_speed: i16,  // type_str_164 (+998) +12
    pub strafe: i16,     // +998 +16
    /// Invulnerability-reset countdown (`word_0x159_345`, +998+345).
    pub invuln: i16,
    /// The WANTED timer (`word_0x248_584`, +998+584) — village aggro.
    pub wanted: i16,
    pub hand_left: i16,  // +2103 (SpellIndexLeft; -1 = empty)
    pub hand_right: i16, // +2105
    /// The str_611 spellbook block (block-relative offsets from the
    /// remc2 `_2BDE` comments): banked XP `SpellExperience_0x263`
    /// @+0x649, volatile XP `spellsExperience_0x2CB` @+0x6B1,
    /// manifestation pool slots `SpellsEnabled_0x333` @+0x719 (u16,
    /// 0 = not learned), cycle rings `array_0x3B5` @+0x79B, levels
    /// `SpellLevels_0x41D` @+0x803, selected tiers `array_0x437`
    /// @+0x81D. All keyed by spell index 0..25.
    pub xp_bank: [i32; 26],
    pub xp_vol: [i32; 26],
    pub spell_ent: [u16; 26],
    pub ring: [u8; 26],
    pub levels: [u8; 26],
    pub sel: [u8; 26],
}

/// The typed MC2 retail closure the conformance importer consumes.
#[derive(Debug, Clone)]
pub struct RetailMc2 {
    pub rand: u32,
    pub local_player: u16,
    pub player_count: u16,
    /// Per-model spawn ordinals (`array_0x10`, struct +0x10..0x2D) —
    /// the phase stagger MC2 class-5 ctors store into `byte_0x3E_62`.
    pub spawn_ord: [u8; 29],
    pub players: Vec<RetailPlayerMc2>,
    /// All 1000 pool records, indexed by slot (slot 0 included).
    pub ents: Vec<RetailEntMc2>,
    /// The LIVE free-slot stack: top index `dword_0x35` (−1 = empty;
    /// NOT the dead `dword_0x242`), pointer cells @+0x246. Retail pops
    /// `cells[top--]` (`NewEvent_4A050`, Events.cpp:561) and the load
    /// rebuild pushes by DESCENDING slot scan 999→1 (`sub_49F90`), so
    /// the top is the lowest-numbered free slot. Bottom-up here; the
    /// LAST element is the next allocation.
    pub free_stack: Vec<u16>,
    /// The recycle-victim stack (live but sacrificable entities,
    /// `flags byte[2] & 2`): top `dword_0x11E6`, cells @+0x11EA.
    /// Popped only when the free stack is EXHAUSTED (free-first —
    /// the opposite of MC1's recycle-first order).
    pub recycle_stack: Vec<u16>,
    /// Level id from the embedded level record (+0x2FED0).
    pub level: u16,
    /// The saved `type_str_160* dword_0x36DF6` — points at the
    /// behavior table's default row `&str_D7BD6[59]` at save time, so
    /// an entity's absolute behavior row is
    /// `(ptr_a0 − base160)/34 + 59` (retail's own load fixup,
    /// Level.cpp:1255-57).
    pub base160: u32,
    /// The LIVE StageVar table `StageVars2_0x365F4[11]` (LS:249), raw
    /// 8-byte rows: [kind, flags, chain, cadence, payload×4]. Runtime
    /// lanes (FIRED &4, kind-7 arm &0x18, the cadence counter, kind-6
    /// timer) mutate here; on &2-clear watch rows the payload can
    /// become a bound-entity guest POINTER (EF:4740) — consumers must
    /// range-guard before reading it as a value.
    pub stagevars: [[u8; 8]; 11],
}

pub fn decode_retail_ent_mc2(d: &[u8], slot: u16) -> RetailEntMc2 {
    let o = m2::POOL + slot as usize * m2::ENT_STRIDE;
    let mut mail = [(0i32, 0u16); 6];
    for (ch, m) in mail.iter_mut().enumerate() {
        *m = (i32_(d, o + 0x5E + ch * 6), u16_(d, o + 0x62 + ch * 6));
    }
    RetailEntMc2 {
        next0: u32_(d, o),
        max_life: i32_(d, o + 0x04),
        life: i32_(d, o + 0x08),
        flags: u32_(d, o + 0x0C),
        scratch10: i32_(d, o + 0x10),
        rand: u16_(d, o + 0x14),
        f16: u16_(d, o + 0x16),
        next18: u16_(d, o + 0x18),
        f1a: u16_(d, o + 0x1A),
        yaw: i16_(d, o + 0x1C),
        pitch: i16_(d, o + 0x1E),
        roll: i16_(d, o + 0x20),
        f22: i16_(d, o + 0x22),
        f24: i16_(d, o + 0x24),
        f26: i16_(d, o + 0x26),
        owner28: u16_(d, o + 0x28),
        f2a: u16_(d, o + 0x2A),
        f2c: i16_(d, o + 0x2C),
        f2e: i16_(d, o + 0x2E),
        f30: u16_(d, o + 0x30),
        f32: u16_(d, o + 0x32),
        f34: u16_(d, o + 0x34),
        f36: u16_(d, o + 0x36),
        b38: i8_(d, o + 0x38),
        b39: i8_(d, o + 0x39),
        b3a: i8_(d, o + 0x3A),
        b3b: i8_(d, o + 0x3B),
        b3c: i8_(d, o + 0x3C),
        b3d: i8_(d, o + 0x3D),
        phase3e: u8_(d, o + 0x3E),
        class3f: u8_(d, o + 0x3F),
        model40: u8_(d, o + 0x40),
        b41: i8_(d, o + 0x41),
        b42: i8_(d, o + 0x42),
        b43: i8_(d, o + 0x43),
        b44: i8_(d, o + 0x44),
        action45: u8_(d, o + 0x45),
        b46: i8_(d, o + 0x46),
        b47: i8_(d, o + 0x47),
        sv1: i8_(d, o + 0x48),
        sv2: i8_(d, o + 0x49),
        sv_timer: i16_(d, o + 0x4A),
        x: u16_(d, o + 0x4C),
        y: u16_(d, o + 0x4E),
        z: i16_(d, o + 0x50),
        ayaw: i16_(d, o + 0x52),
        apitch: i16_(d, o + 0x54),
        aroll: i16_(d, o + 0x56),
        afov: i16_(d, o + 0x58),
        f5a: i16_(d, o + 0x5A),
        b5c: i8_(d, o + 0x5C),
        b5d: i8_(d, o + 0x5D),
        mail,
        speed: i16_(d, o + 0x82),
        min_speed: i16_(d, o + 0x84),
        max_speed: i16_(d, o + 0x86),
        d88: i32_(d, o + 0x88),
        mana_max: i32_(d, o + 0x8C),
        mana: i32_(d, o + 0x90),
        player_ent: u16_(d, o + 0x94),
        target96: u16_(d, o + 0x96),
        f98: u16_(d, o + 0x98),
        dest_x: u16_(d, o + 0x9A),
        dest_y: u16_(d, o + 0x9C),
        dest_z: i16_(d, o + 0x9E),
        ptr_a0: u32_(d, o + 0xA0),
        ptr_a4: u32_(d, o + 0xA4),
    }
}

fn decode_retail_player_mc2(d: &[u8], i: u16) -> RetailPlayerMc2 {
    let b = m2::PLAYERS + i as usize * m2::PLAYER_STRIDE;
    let t = b + m2::PP_FLIGHT;
    let mut xp_bank = [0i32; 26];
    let mut xp_vol = [0i32; 26];
    let mut spell_ent = [0u16; 26];
    let mut ring = [0u8; 26];
    let mut levels = [0u8; 26];
    let mut sel = [0u8; 26];
    for s in 0..26 {
        xp_bank[s] = i32_(d, b + 0x649 + s * 4);
        xp_vol[s] = i32_(d, b + 0x6B1 + s * 4);
        spell_ent[s] = u16_(d, b + 0x719 + s * 2);
        ring[s] = u8_(d, b + 0x79B + s);
        levels[s] = u8_(d, b + 0x803 + s);
        sel[s] = u8_(d, b + 0x81D + s);
    }
    RetailPlayerMc2 {
        flags: u32_(d, b),
        is_ai: u8_(d, b + m2::PP_ISAI) != 0,
        play_index: u16_(d, b + m2::PP_PLAYINDEX),
        turn: i32_(d, b + m2::PP_TURN),
        castle: i16_(d, b + m2::PP_CASTLE),
        cmd_speed: i16_(d, t + 12),
        strafe: i16_(d, t + 16),
        invuln: i16_(d, t + 345),
        wanted: i16_(d, t + 584),
        hand_left: i16_(d, b + m2::PP_HAND_L),
        hand_right: i16_(d, b + m2::PP_HAND_R),
        xp_bank,
        xp_vol,
        spell_ent,
        ring,
        levels,
        sel,
    }
}

/// Decode the full MC2 retail closure from a raw struct image.
pub fn decode_retail_mc2(d: &[u8]) -> Result<RetailMc2, String> {
    if d.len() != MC2_STRUCT_SIZE {
        return Err(format!(
            "MC2 struct image is {} bytes, want {MC2_STRUCT_SIZE}",
            d.len()
        ));
    }
    // Entity stacks: pointer cells → slots. D41A0_0 is a static
    // global, so the guest base is recovered from any live cell's
    // pool-stride residue rather than hardcoded per build.
    let pcount = u16_(d, m2::LOCAL_PLAYER + 2);
    let mut spawn_ord = [0u8; 29];
    spawn_ord.copy_from_slice(&d[0x10..0x10 + 29]);
    Ok(RetailMc2 {
        rand: u32_(d, m2::RNG),
        local_player: u16_(d, m2::LOCAL_PLAYER),
        player_count: pcount,
        spawn_ord,
        players: (0..pcount.min(8))
            .map(|i| decode_retail_player_mc2(d, i))
            .collect(),
        ents: (0..m2::ENT_COUNT as u16)
            .map(|s| decode_retail_ent_mc2(d, s))
            .collect(),
        free_stack: mc2_stack(d, 0x35, 0x246),
        recycle_stack: mc2_stack(d, 0x11E6, 0x11EA),
        level: u16_(d, m2::POOL + m2::ENT_COUNT * m2::ENT_STRIDE + 2),
        base160: u32_(d, 0x36DF6),
        stagevars: {
            let mut sv = [[0u8; 8]; 11];
            for (i, row) in sv.iter_mut().enumerate() {
                row.copy_from_slice(&d[0x365F4 + i * 8..0x365F4 + i * 8 + 8]);
            }
            sv
        },
    })
}

/// Decode an MC2 entity stack (top dword + guest-pointer cells into
/// the pool). `D41A0_0` is a static global, but DOS/4GW's load delta
/// makes the guest base run-specific — recover it from the cells:
/// the pool base is `cells[0] − s·168` for the (unique) `s` under
/// which EVERY cell lands on a stride boundary inside the pool.
/// Validated against mc2l0: top@0x35 == free-census − 1 and the
/// decoded set is exactly the class-0 slots. Returns slots bottom-up
/// (last = next allocation).
fn mc2_stack(d: &[u8], top_off: usize, cells_off: usize) -> Vec<u16> {
    let top = i32_(d, top_off) as i64;
    if !(0..1000).contains(&top) {
        return Vec::new();
    }
    let stride = m2::ENT_STRIDE as u32;
    let cells: Vec<u32> = (0..=top as usize)
        .map(|i| u32_(d, cells_off + i * 4))
        .collect();
    let base = (0..1000u32)
        .rev()
        .map(|s| cells[0].wrapping_sub(s * stride))
        .find(|&cand| {
            cells.iter().all(|&p| {
                let rel = p.wrapping_sub(cand);
                rel % stride == 0 && (rel / stride) < 1000
            })
        });
    let Some(base) = base else { return Vec::new() };
    cells
        .iter()
        .map(|&p| (p.wrapping_sub(base) / stride) as u16)
        .collect()
}

/// Decode the full MC1 retail closure from a raw struct image.
pub fn decode_retail_mc1(d: &[u8]) -> Result<RetailMc1, String> {
    if d.len() != MC1_STRUCT_SIZE {
        return Err(format!(
            "MC1 struct image is {} bytes, want {MC1_STRUCT_SIZE}",
            d.len()
        ));
    }
    let mut spawn_count = [0u8; 20];
    spawn_count.copy_from_slice(&d[12..32]);
    // Free/recycle stacks: pointer cells → slots. The struct's heap
    // guest base is fixed (0x1DE40, both builds); a cell that does
    // not convert cleanly ends the stack (stale garbage sits above
    // the live top from earlier pops).
    const STRUCT_GUEST: u32 = 0x1DE40;
    let cell_slot = |ptr: u32| -> Option<u16> {
        let rel = ptr.checked_sub(STRUCT_GUEST + m1::POOL as u32)?;
        (rel % m1::ENT_STRIDE as u32 == 0 && (rel / m1::ENT_STRIDE as u32) < 1000)
            .then_some((rel / m1::ENT_STRIDE as u32) as u16)
    };
    let stack = |top: i64, cells_off: usize| -> Vec<u16> {
        if !(0..1000).contains(&top) {
            return Vec::new();
        }
        (0..=top as usize)
            .filter_map(|i| cell_slot(u32_(d, cells_off + i * 4)))
            .collect()
    };
    let free_stack = stack(u32_(d, 40) as i64, 593);
    let recycle_stack = stack(i32_(d, 4_593) as i64, 4_597);
    Ok(RetailMc1 {
        rand: u32_(d, m1::RNG),
        local_player: u16_(d, m1::LOCAL_PLAYER),
        player_count: u16_(d, m1::LOCAL_PLAYER + 2),
        spawn_count,
        wizards: (0..m1::WIZARD_COUNT as u16)
            .map(|i| decode_retail_wizard_mc1(d, i))
            .collect(),
        ents: (0..m1::ENT_COUNT as u16)
            .map(|s| decode_retail_ent_mc1(d, s))
            .collect(),
        free_stack,
        recycle_stack,
        level: u16_(d, 232_707),
    })
}
