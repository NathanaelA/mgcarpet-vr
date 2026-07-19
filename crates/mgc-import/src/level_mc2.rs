//! Magic Carpet 2 level format (LEVELS.DAT entries, decompressed).
//!
//! Layout per michaelhoward's MC2 spec, cross-verified against
//! remc2's `Type_Level_2FECE` / `Type_CompressedLevel_2FECE`
//! (BasicTerrain.h, #pragma pack 1) and all 165 standard retail levels:
//!
//! ```text
//! 0x0000     23  header: version(u16)=2, level id(u16), gfx type(u8),
//!                basic height(u8: the cave ceiling mirror pivot,
//!                byte_0x2FED3), map type(u8: 0 day/1 night/2 cave),
//!                unk07(i16), unk09(i16), players(i8[8]), pad[4]
//! 0x0017     46  terrain generation params: 12 values, each u16 LE +
//!                2 pad bytes, EXCEPT River (true u32 LE) and the final
//!                RkSte (bare u16, no padding — hence 46 bytes, not 48)
//! 0x0045   1022  reserved (all zeros)
//! 0x0443  24000  entity table: 1200 slots x 20 bytes, 10 x u16 LE.
//!                NOTE: michaelhoward's spec claims these are big-endian;
//!                retail GOG data is demonstrably little-endian (verified
//!                against all 165 levels — BE reads put every coordinate
//!                off-grid). Possibly true of some other release, or a
//!                spec error.
//! 0x6203      1  separator byte
//! 0x6204    880  wizard configuration: 8 blocks x 110 bytes
//! 0x6574     56  stage checkpoints: 8 x 7 bytes (mission script)
//! 0x65AC     88  stage variables: 11 x 8 bytes (mission script)
//! total   26116  (0x6604)
//! ```
//!
//! Unused stage entries are 0xFF-filled.

pub const MC2_LEVEL_SIZE: usize = 26116;
pub const THING_SLOTS: usize = 1200;
pub const WIZARD_SLOTS: usize = 8;
pub const CHECKPOINT_SLOTS: usize = 8;
pub const STAGE_VAR_SLOTS: usize = 11;
pub const SPELL_COUNT: usize = 26;

const GENPARAMS_OFFSET: usize = 0x17;
const RESERVED_OFFSET: usize = 0x45;
const THINGS_OFFSET: usize = 0x443;
const SEPARATOR_OFFSET: usize = 0x6203;
const WIZARDS_OFFSET: usize = 0x6204;
const WIZARD_SIZE: usize = 110;
const CHECKPOINTS_OFFSET: usize = 0x6574;
const STAGE_VARS_OFFSET: usize = 0x65AC;
const THING_SIZE: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Mc2LevelError {
    /// Input is not exactly [`MC2_LEVEL_SIZE`] bytes (the 18 "extended"
    /// dev-leftover levels at archive indices 160+ are a different,
    /// older format and are rejected here).
    BadSize(usize),
    /// Header version field is not 2.
    BadVersion(u16),
}

impl std::fmt::Display for Mc2LevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSize(n) => write!(f, "MC2 level must be {MC2_LEVEL_SIZE} bytes, got {n}"),
            Self::BadVersion(v) => write!(f, "MC2 level version must be 2, got {v}"),
        }
    }
}

impl std::error::Error for Mc2LevelError {}

/// Environment type; selects the entire asset set (sprites, sky,
/// palette, tables, blocks) the original engine loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    Day,
    Night,
    Cave,
    /// Value outside 0..=2 (not observed in retail data).
    Unknown(u8),
}

impl MapType {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::Day,
            1 => Self::Night,
            2 => Self::Cave,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Mc2Header {
    pub version: u16,
    pub level_id: u16,
    pub gfx_type: u8,
    /// Cave basic height (byte_0x2FED3 — the ceiling mirror pivot on
    /// cave levels).
    pub basic_height: u8,
    pub map_type: MapType,
    /// `word_0x2FED5` — authored initial value of a field retail
    /// REPURPOSES at runtime as the current-objective scratch word
    /// (EF:40573/40760-61); the authored value (10/93/100 observed)
    /// has no identified load-time consumer.
    pub unk07: i16,
    /// `word_0x2FED7` = **NumberOfPlayers** (EF:39382/39461 →
    /// `NumberOfPlayers_0xe`): colors `0..n-1` spawn wizard carpets
    /// (the input pump that consumes the spawn enqueue is bounded by
    /// it — docs/traces/mc2-rivals-spawn-mortality.md §1). Color 0 =
    /// the human in single player (`LevelIndex_0xc = 0`, EF:43127);
    /// 1..n-1 = AI rivals. Retail name pending a field rename (the
    /// serde key rides a BAKE_EPOCH bump).
    pub unk09: i16,
    /// `player_0x2FED9[8]` — authored starting-castle LEVEL per wizard
    /// color (0 = none, N = a castle at level N-1 built at the
    /// wizard's spawn; consumers EF:43777/43789, docs/traces/
    /// mc2-castle-data-tables.md §3). NOT "activation flags".
    pub players: [i8; 8],
}

/// Terrain generation parameters. Same family as MC1's GEN_MAP with one
/// addition (`lriver`); field names are Bullfrog's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mc2GenMap {
    pub seed: u16,
    pub off: u16,
    pub raise: i16,
    pub gnarl: u16,
    pub river: u32,
    pub lriver: u16,
    pub sourc: u16,
    pub snlin: u16,
    pub snflt: u16,
    pub bhlin: u16,
    pub bhflt: u16,
    pub rkste: u16,
}

/// One 20-byte entity record (little-endian on disk in GOG data,
/// despite the spec's big-endian claim — see module docs).
///
/// MC1 field correspondence: `word10` sits where MC1 kept SwiSz,
/// `stage_tag` where SwiId (repurposed for the mission system but still
/// switch/trigger linkage), `par1`/`par2` where Parent/Child (and still
/// used as such by Path/Wall/Canyon chains); `par3` is new.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mc2Thing {
    pub class: u16,
    pub model: u16,
    pub x: u16,
    pub y: u16,
    pub dis_id: i16,
    pub word10: u16,
    pub stage_tag: i16,
    pub par1: u16,
    pub par2: u16,
    pub par3: u16,
}

impl Mc2Thing {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// MC2 level files use classes 0..=15 (class 0 = Conditional Spawn —
    /// real content, unlike MC1's class-0 markers). Anything outside, or
    /// with impossible high bytes, is garbage.
    pub fn is_active(&self) -> bool {
        !self.is_empty() && self.class <= 15 && self.model < 256 && self.x < 256 && self.y < 256
    }
}

/// One 110-byte wizard configuration block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WizardSettings {
    pub aggression: i16,
    pub reflexes: i16,
    pub perception: i16,
    /// `StartingSpells_0x360E1x` — per-spell GRANT FLAG, indexed by
    /// spell ID (0=Fireball .. 25=Cave In). Consumed by
    /// `InitialiseSpells_54A50` (EF:38650).
    pub starting_spells: [u8; SPELL_COUNT],
    /// `byte_0x360FBx` — per-spell STARTING XP LEVEL 0..2: for an AI
    /// wizard, `SpellLevels[spell] = min(this, 2)` at book init
    /// (EF:38693; docs/traces/mc2-rivals-spawn-mortality.md §3). Field
    /// rename rides a BAKE_EPOCH bump (serde key compat).
    pub unknown_spells: [u8; SPELL_COUNT],
    /// `BlockedSpells_0x36115x` — per-spell DENY flag.
    pub blocked_spells: [u8; SPELL_COUNT],
    /// `Life_0x3612F` — AI life scale (16.8; also scales maxLife,
    /// EF:43768-71). Human always 256 (EF:43720).
    pub life: i16,
}

/// Stage checkpoint (mission-script entry). Unused slots are 0xFF-filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageCheckpoint {
    pub index: i8,
    pub stage: i16,
    pub x: i16,
    pub y: i16,
}

impl StageCheckpoint {
    pub fn is_used(&self) -> bool {
        self.index >= 0
    }
}

/// Stage variable (mission-script state). Unused slots are 0xFF-filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageVar {
    pub index: i8,
    pub stage: i8,
    pub x: u8,
    pub y: u8,
    pub data: u32,
}

impl StageVar {
    /// A row is live when it isn't the editor's 0xFF fill and its
    /// KIND nibble is nonzero. byte0's HIGH bits are flags (0x80
    /// subtype-match, 0x40 watch-model), so a signed `>= 0` test
    /// wrongly discards flagged rows — test the nibble, not the sign.
    pub fn is_used(&self) -> bool {
        (self.index as u8) != 0xFF && (self.index as u8) & 0xF != 0
    }
}

#[derive(Debug)]
pub struct Mc2Level {
    pub header: Mc2Header,
    pub gen_map: Mc2GenMap,
    /// All 1200 slots, preserving indices.
    pub things: Vec<Mc2Thing>,
    pub separator: u8,
    pub wizards: [WizardSettings; WIZARD_SLOTS],
    pub checkpoints: [StageCheckpoint; CHECKPOINT_SLOTS],
    pub stage_vars: [StageVar; STAGE_VAR_SLOTS],
    pub reserved_nonzero: bool,
}

impl Mc2Level {
    pub fn parse(data: &[u8]) -> Result<Self, Mc2LevelError> {
        if data.len() != MC2_LEVEL_SIZE {
            return Err(Mc2LevelError::BadSize(data.len()));
        }
        let u16le = |o: usize| u16::from_le_bytes(data[o..o + 2].try_into().unwrap());
        let u32le = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());

        let version = u16le(0x00);
        if version != 2 {
            return Err(Mc2LevelError::BadVersion(version));
        }

        let header = Mc2Header {
            version,
            level_id: u16le(0x02),
            gfx_type: data[0x04],
            basic_height: data[0x05],
            map_type: MapType::from_byte(data[0x06]),
            unk07: u16le(0x07) as i16,
            unk09: u16le(0x09) as i16,
            players: std::array::from_fn(|i| data[0x0B + i] as i8),
        };

        // 12 params, u16 + 2 pad each, except River (u32) and the final
        // RkSte (bare u16) — see module docs.
        let g = GENPARAMS_OFFSET;
        let gen_map = Mc2GenMap {
            seed: u16le(g),
            off: u16le(g + 0x04),
            raise: u16le(g + 0x08) as i16,
            gnarl: u16le(g + 0x0C),
            river: u32le(g + 0x10),
            lriver: u16le(g + 0x14),
            sourc: u16le(g + 0x18),
            snlin: u16le(g + 0x1C),
            snflt: u16le(g + 0x20),
            bhlin: u16le(g + 0x24),
            bhflt: u16le(g + 0x28),
            rkste: u16le(g + 0x2C),
        };

        let reserved_nonzero = data[RESERVED_OFFSET..THINGS_OFFSET].iter().any(|&b| b != 0);

        let mut things = Vec::with_capacity(THING_SLOTS);
        for slot in 0..THING_SLOTS {
            let o = THINGS_OFFSET + slot * THING_SIZE;
            things.push(Mc2Thing {
                class: u16le(o),
                model: u16le(o + 2),
                x: u16le(o + 4),
                y: u16le(o + 6),
                dis_id: u16le(o + 8) as i16,
                word10: u16le(o + 10),
                stage_tag: u16le(o + 12) as i16,
                par1: u16le(o + 14),
                par2: u16le(o + 16),
                par3: u16le(o + 18),
            });
        }

        let wizards = std::array::from_fn(|i| {
            let o = WIZARDS_OFFSET + i * WIZARD_SIZE;
            WizardSettings {
                aggression: u16le(o + 0x03) as i16,
                reflexes: u16le(o + 0x07) as i16,
                perception: u16le(o + 0x0B) as i16,
                starting_spells: data[o + 0x0F..o + 0x0F + SPELL_COUNT].try_into().unwrap(),
                unknown_spells: data[o + 0x29..o + 0x29 + SPELL_COUNT].try_into().unwrap(),
                blocked_spells: data[o + 0x43..o + 0x43 + SPELL_COUNT].try_into().unwrap(),
                life: u16le(o + 0x5D) as i16,
            }
        });

        let checkpoints = std::array::from_fn(|i| {
            let o = CHECKPOINTS_OFFSET + i * 7;
            StageCheckpoint {
                index: data[o] as i8,
                stage: u16le(o + 1) as i16,
                x: u16le(o + 3) as i16,
                y: u16le(o + 5) as i16,
            }
        });

        let stage_vars = std::array::from_fn(|i| {
            let o = STAGE_VARS_OFFSET + i * 8;
            StageVar {
                index: data[o] as i8,
                stage: data[o + 1] as i8,
                x: data[o + 2],
                y: data[o + 3],
                data: u32le(o + 4),
            }
        });

        Ok(Self {
            header,
            gen_map,
            things,
            separator: data[SEPARATOR_OFFSET],
            wizards,
            checkpoints,
            stage_vars,
            reserved_nonzero,
        })
    }

    /// Real placed entities with their slot indices.
    pub fn active_things(&self) -> impl Iterator<Item = (usize, &Mc2Thing)> {
        self.things
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_active())
    }

    /// Non-empty slots that fail structural sanity (garbage).
    pub fn junk(&self) -> impl Iterator<Item = (usize, &Mc2Thing)> {
        self.things
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_empty() && !t.is_active())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_level() -> Vec<u8> {
        let mut data = vec![0u8; MC2_LEVEL_SIZE];
        data[0x00..0x02].copy_from_slice(&2u16.to_le_bytes());
        // ff-fill the stage tables like retail data.
        for b in &mut data[CHECKPOINTS_OFFSET..MC2_LEVEL_SIZE] {
            *b = 0xFF;
        }
        data
    }

    #[test]
    fn rejects_wrong_size() {
        assert_eq!(
            Mc2Level::parse(&[0u8; 100]).err().unwrap(),
            Mc2LevelError::BadSize(100)
        );
    }

    #[test]
    fn rejects_wrong_version() {
        let mut data = blank_level();
        data[0x00] = 1;
        assert_eq!(
            Mc2Level::parse(&data).err().unwrap(),
            Mc2LevelError::BadVersion(1)
        );
    }

    #[test]
    fn parses_synthetic_level() {
        let mut data = blank_level();
        data[0x06] = 2; // cave
        // raise = -10000 at genparams offset 0x08 (i16 LE + padding).
        data[0x17 + 0x08..0x17 + 0x0A].copy_from_slice(&(-10000i16).to_le_bytes());
        // river (true u32) = 70000.
        data[0x17 + 0x10..0x17 + 0x14].copy_from_slice(&70000u32.to_le_bytes());
        // rkste is the final bare u16 right before the reserved block.
        data[0x43..0x45].copy_from_slice(&77u16.to_le_bytes());
        // Entity in slot 5: class 5 model 20 (Spider) at (100, 200).
        let o = THINGS_OFFSET + 5 * THING_SIZE;
        data[o..o + 2].copy_from_slice(&5u16.to_le_bytes());
        data[o + 2..o + 4].copy_from_slice(&20u16.to_le_bytes());
        data[o + 4..o + 6].copy_from_slice(&100u16.to_le_bytes());
        data[o + 6..o + 8].copy_from_slice(&200u16.to_le_bytes());
        // Wizard 0: aggression 128, life 500.
        let w = WIZARDS_OFFSET;
        data[w + 0x03..w + 0x05].copy_from_slice(&128u16.to_le_bytes());
        data[w + 0x5D..w + 0x5F].copy_from_slice(&500u16.to_le_bytes());
        // One used checkpoint: index 0, stage 1, at (115, 212).
        let c = CHECKPOINTS_OFFSET;
        data[c] = 0;
        data[c + 1..c + 3].copy_from_slice(&1u16.to_le_bytes());
        data[c + 3..c + 5].copy_from_slice(&115u16.to_le_bytes());
        data[c + 5..c + 7].copy_from_slice(&212u16.to_le_bytes());

        let level = Mc2Level::parse(&data).unwrap();
        assert_eq!(level.header.map_type, MapType::Cave);
        assert_eq!(level.gen_map.raise, -10000);
        assert_eq!(level.gen_map.river, 70000);
        assert_eq!(level.gen_map.rkste, 77);

        let active: Vec<_> = level.active_things().collect();
        assert_eq!(active.len(), 1);
        let (slot, thing) = active[0];
        assert_eq!(slot, 5);
        assert_eq!(
            (thing.class, thing.model, thing.x, thing.y),
            (5, 20, 100, 200)
        );

        assert_eq!(level.wizards[0].aggression, 128);
        assert_eq!(level.wizards[0].life, 500);

        let used: Vec<_> = level.checkpoints.iter().filter(|c| c.is_used()).collect();
        assert_eq!(used.len(), 1);
        assert_eq!((used[0].stage, used[0].x, used[0].y), (1, 115, 212));
        assert!(level.stage_vars.iter().all(|v| !v.is_used()));
    }
}
