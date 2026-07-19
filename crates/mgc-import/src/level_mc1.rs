//! Magic Carpet 1 level format (LEV*.DAT, decompressed).
//!
//! Layout follows the remc1 decompile's own field accessors (the
//! engine copies the whole record to str_193795 and address-arithmetic
//! names every field), which correct michaelhoward's
//! MagicCarpetFileFormat spec: the spec's "2095-slot entity table +
//! 12-byte footer" actually ends at offset 37072 — the last 96
//! pseudo-slots are the 8 x 216-byte per-player WIZARD records
//! (str_230867_37072, remc1 :49222/:54965-67), and the "footer" is the
//! decoded tail (u16 map-coord word :27268, u16 player count :51537,
//! u8[8] per-player starting castle levels :54972-94). Cross-checked
//! against all 143 levels in the GOG Magic Carpet Plus data (70
//! campaign + 73 Hidden Worlds).
//!
//! ```text
//! 0x0000   48     GEN_MAP header: 12 x u32 LE (pre-header + 11 terrain
//!                 generation parameters; terrain is seed-generated, no
//!                 stored heightmap)
//! 0x0030   1042   reserved (all zeros in known levels; the engine's
//!                 str_1072 slot 0 [1072..1090] is its runtime SCRATCH
//!                 record, never authored)
//! 0x0442   35982  entity table: 1999 slots x 18 bytes (THING_INIT,
//!                 engine slots 1..=1999)
//! 0x90D0   1728   wizard configs: 8 players x 216 bytes
//! 0x9790   2      u16, map-screen coordinate math word (:27268)
//! 0x9792   2      u16, active player (wizard) count
//! 0x9794   8      u8[8], per-player starting castle level (0 = none,
//!                 N = spawn the player's castle at level N-1)
//! total    38812  (0x979C)
//! ```

pub const MC1_LEVEL_SIZE: usize = 38812;
pub const THING_SLOTS: usize = 1999;

const RESERVED_OFFSET: usize = 0x0030;
const THINGS_OFFSET: usize = 0x0442;
const WIZARDS_OFFSET: usize = 0x90D0;
const WIZARD_SIZE: usize = 216;
const FOOTER_OFFSET: usize = 0x9790;
const THING_SIZE: usize = 18;

#[derive(Debug, PartialEq, Eq)]
pub enum Mc1LevelError {
    /// Input is not exactly [`MC1_LEVEL_SIZE`] bytes.
    BadSize(usize),
}

impl std::fmt::Display for Mc1LevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSize(n) => {
                write!(f, "MC1 level must be {MC1_LEVEL_SIZE} bytes, got {n}")
            }
        }
    }
}

impl std::error::Error for Mc1LevelError {}

/// Terrain generation parameters (the `GEN_MAP` block). Field names are
/// Bullfrog's own, recovered from the GAM*.DAT save-game text format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenMap {
    /// Pre-header field, not part of GEN_MAP proper; purpose unknown.
    pub pre_header: u32,
    /// Terrain generation seed.
    pub seed: u32,
    /// Base elevation offset.
    pub off: u32,
    /// Global raise amount; negative = sunken below sea level.
    pub raise: i32,
    /// Fractal roughness, 0 (smooth) ..= 128 (extreme).
    pub gnarl: u32,
    /// Number of river channels.
    pub river: u32,
    /// Number of river source points.
    pub sourc: u32,
    /// Snow line height threshold.
    pub snlin: u32,
    /// Snow blending parameter.
    pub snflt: u32,
    /// Beach line height threshold.
    pub bhlin: u32,
    /// Beach blending parameter.
    pub bhflt: u32,
    /// Rock steepness threshold.
    pub rkste: u32,
}

/// One entity slot (`THING_INIT` record). All-zero slots are empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThingInit {
    pub class: u16,
    pub model: u16,
    pub x: u16,
    pub y: u16,
    /// Disposition ID; 0xFFFF = none.
    pub dis_id: u16,
    pub swi_sz: u16,
    /// Switch ID; 0xFFFF = none.
    pub swi_id: u16,
    /// Parent slot index (linked lists: paths, wizard groups).
    pub parent: u16,
    /// Child slot index.
    pub child: u16,
}

/// What a non-trivial table slot actually holds. Retail levels contain
/// more than the spec's entity records:
///
/// - **Entity**: a placed thing with a documented class (2..=15).
/// - **Marker**: class-0 records with plausible structure — observed as
///   sequential coordinate chains (e.g. MC1 level 62 slots 1899+ trace a
///   path point by point) and clusters with consistent DisId/SwiId.
///   Semantics unconfirmed; likely terrain-feature nodes. Not listed in
///   the .INF summaries the spec's counts were derived from.
/// - **Junk**: uninitialized-editor-memory garbage in the table tail —
///   fields made of 0x00/0x01 byte patterns (values 0/1/256/257), and
///   every class-1 record ever observed (46/46 are garbage-shaped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingKind {
    Empty,
    Entity,
    Marker,
    Junk,
}

impl ThingInit {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn kind(&self) -> ThingKind {
        if self.is_empty() {
            return ThingKind::Empty;
        }
        // Bit-garbage detector: impossible high bytes anywhere, classes
        // outside the known range, or class 1 (never a real entity).
        let structurally_sane =
            self.class <= 15 && self.model < 256 && self.x < 256 && self.y < 256;
        if !structurally_sane || self.class == 1 {
            return ThingKind::Junk;
        }
        if self.class == 0 {
            return ThingKind::Marker;
        }
        ThingKind::Entity
    }
}

/// One per-player wizard config record (str_230867_37072[player], 216
/// bytes). Field offsets are the remc1 accessors: +4 -> Type_160
/// u16_522 (:54965), +8 -> u16_526 (:54967), +12 -> u16_524 (:54966),
/// +16 var_230883[24] (:49222), +116 var_230983[24] (:49229). Bytes
/// +0..3, +40..115 and +140..215 are read nowhere in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WizardRecord {
    /// AI aggression (u16_522): hate rise rate, war thresholds,
    /// opportunism margins.
    pub aggression: u16,
    /// AI tempo (u16_526): decision period, turn agility, fireball
    /// burst pause, respawn delay.
    pub tempo: u16,
    /// AI accuracy (u16_524): commit aim cone, rebound-notice
    /// probability.
    pub accuracy: u16,
    /// Pre-granted spell mask (var_230883): with `allowed`, the AI's
    /// level-start spellbook (grant iff both nonzero, :49222).
    pub pregrant: [u8; 24],
    /// Availability mask (var_230983): the same mask the HUMAN grant
    /// intersects with collected flags; also the AI's learn-eligible
    /// list (Type_160+796).
    pub allowed: [u8; 24],
}

#[derive(Debug)]
pub struct Mc1Level {
    pub gen_map: GenMap,
    /// All 1999 authored slots (engine slots 1..=1999 — the engine's
    /// slot 0 is a runtime scratch record), preserving indices
    /// (parent/child reference them).
    pub things: Vec<ThingInit>,
    /// The 8 per-player wizard config records.
    pub wizards: [WizardRecord; 8],
    /// Map-screen coordinate word at 0x9790 (semantics untraced beyond
    /// the map-compose read :27268).
    pub tail_38800: u16,
    /// Active player (wizard) count (engine var_u16_10, :51537).
    pub player_count: u16,
    /// Per-player starting castle level: 0 = none, N = a castle at
    /// level N-1 spawns with the wizard (:54972-94).
    pub castle_levels: [u8; 8],
    /// The raw 12-byte tail as 6 u16s (the spec-named "footer", =
    /// tail_38800/player_count/castle_levels verbatim) — kept for the
    /// shipped GenParams member.
    pub footer: [u16; 6],
    /// True when the reserved block deviates from the all-zeros norm.
    pub reserved_nonzero: bool,
}

impl Mc1Level {
    pub fn parse(data: &[u8]) -> Result<Self, Mc1LevelError> {
        if data.len() != MC1_LEVEL_SIZE {
            return Err(Mc1LevelError::BadSize(data.len()));
        }
        let u32_at = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
        let u16_at = |o: usize| u16::from_le_bytes(data[o..o + 2].try_into().unwrap());

        let gen_map = GenMap {
            pre_header: u32_at(0x00),
            seed: u32_at(0x04),
            off: u32_at(0x08),
            raise: u32_at(0x0C) as i32,
            gnarl: u32_at(0x10),
            river: u32_at(0x14),
            sourc: u32_at(0x18),
            snlin: u32_at(0x1C),
            snflt: u32_at(0x20),
            bhlin: u32_at(0x24),
            bhflt: u32_at(0x28),
            rkste: u32_at(0x2C),
        };

        let reserved_nonzero = data[RESERVED_OFFSET..THINGS_OFFSET].iter().any(|&b| b != 0);

        let mut things = Vec::with_capacity(THING_SLOTS);
        for slot in 0..THING_SLOTS {
            let o = THINGS_OFFSET + slot * THING_SIZE;
            things.push(ThingInit {
                class: u16_at(o),
                model: u16_at(o + 2),
                x: u16_at(o + 4),
                y: u16_at(o + 6),
                dis_id: u16_at(o + 8),
                swi_sz: u16_at(o + 10),
                swi_id: u16_at(o + 12),
                parent: u16_at(o + 14),
                child: u16_at(o + 16),
            });
        }

        let wizards = std::array::from_fn(|p| {
            let o = WIZARDS_OFFSET + p * WIZARD_SIZE;
            WizardRecord {
                aggression: u16_at(o + 4),
                tempo: u16_at(o + 8),
                accuracy: u16_at(o + 12),
                pregrant: std::array::from_fn(|s| data[o + 16 + s]),
                allowed: std::array::from_fn(|s| data[o + 116 + s]),
            }
        });

        let footer = std::array::from_fn(|i| u16_at(FOOTER_OFFSET + i * 2));
        let castle_levels = std::array::from_fn(|p| data[FOOTER_OFFSET + 4 + p]);

        Ok(Self {
            gen_map,
            things,
            wizards,
            tail_38800: u16_at(FOOTER_OFFSET),
            player_count: u16_at(FOOTER_OFFSET + 2),
            castle_levels,
            footer,
            reserved_nonzero,
        })
    }

    fn of_kind(&self, kind: ThingKind) -> impl Iterator<Item = (usize, &ThingInit)> {
        self.things
            .iter()
            .enumerate()
            .filter(move |(_, t)| t.kind() == kind)
    }

    /// Placed entities (class 2..=15) with their slot indices.
    pub fn active_things(&self) -> impl Iterator<Item = (usize, &ThingInit)> {
        self.of_kind(ThingKind::Entity)
    }

    /// Class-0 marker/node records (see [`ThingKind::Marker`]).
    pub fn markers(&self) -> impl Iterator<Item = (usize, &ThingInit)> {
        self.of_kind(ThingKind::Marker)
    }

    /// Garbage slots, kept accessible for research.
    pub fn junk(&self) -> impl Iterator<Item = (usize, &ThingInit)> {
        self.of_kind(ThingKind::Junk)
    }
}

/// Human-readable name for a class/model pair, per the spec's mapping
/// (88 of 103 types identified). Unknowns render as `class/model`.
pub fn thing_name(class: u16, model: u16) -> String {
    let known = match (class, model) {
        (2, 0) => Some("Tree"),
        (2, 1) => Some("Standing stone"),
        (2, 2) => Some("Dolmen"),
        (2, 3) => Some("Bad Stone"),
        (3, 4..=11) => Some("Player start"),
        (5, 0) => Some("Dragon"),
        (5, 1) => Some("Vulture"),
        (5, 2) => Some("Bee"),
        (5, 3) => Some("Worm"),
        (5, 4) => Some("Archer"),
        (5, 5) => Some("Crab"),
        (5, 6) => Some("Kraken"),
        (5, 7) => Some("Troll"),
        (5, 8) => Some("Griffon"),
        (5, 9) => Some("Skeleton"),
        (5, 10) => Some("Emu"),
        (5, 11) => Some("Genie"),
        (5, 12) => Some("Builder"),
        (5, 13) => Some("Townie"),
        (5, 14) => Some("Trader"),
        (5, 16) => Some("Wyvern"),
        (7, 4) => Some("Wind"),
        (10, 0) => Some("Explosion"),
        (10, 1) => Some("Big explosion"),
        (10, 5) => Some("Splash"),
        (10, 6) => Some("Fire"),
        (10, 8) => Some("Mini volcano"),
        (10, 9) => Some("Volcano"),
        (10, 11) => Some("Crater"),
        (10, 13) => Some("White smoke"),
        (10, 14) => Some("Black smoke"),
        (10, 15) => Some("Earthquake"),
        (10, 17) => Some("Meteor"),
        (10, 23) => Some("Lightning"),
        (10, 24) => Some("Rain of fire"),
        (10, 25) => Some("Steal mana"),
        (10, 28) => Some("Wall"),
        (10, 29) => Some("Path"),
        (10, 31) => Some("Canyon"),
        (10, 34) => Some("Teleport"),
        (10, 39) => Some("Mana ball"),
        (10, 45) => Some("Wizard"),
        (10, 50) => Some("Ridge node"),
        (10, 52) => Some("Crab egg"),
        (11, _) => Some("Switch"),
        (12, 0) => Some("Fireball pickup"),
        (12, 1) => Some("Heal pickup"),
        (12, 2) => Some("Alliance pickup"),
        (12, 3) => Some("Possession pickup"),
        (12, 4) => Some("Shield pickup"),
        (12, 5) => Some("Beyond sight pickup"),
        (12, 6) => Some("Earthquake pickup"),
        (12, 7) => Some("Meteor pickup"),
        (12, 8) => Some("Volcano pickup"),
        (12, 9) => Some("Crater pickup"),
        (12, 10) => Some("Teleport pickup"),
        (12, 11) => Some("Rubber band pickup"),
        (12, 12) => Some("Invisible pickup"),
        (12, 13) => Some("Steal mana pickup"),
        (12, 14) => Some("Rebound pickup"),
        (12, 15) => Some("Lightning pickup"),
        (12, 16) => Some("Castle pickup"),
        (12, 17) => Some("Skeleton pickup"),
        (12, 18) => Some("Thunder bolt pickup"),
        (12, 19) => Some("Mana magnet pickup"),
        (12, 20) => Some("Fire wall pickup"),
        (12, 21) => Some("Reverse speed pickup"),
        (12, 22) => Some("Smart bomb pickup"),
        (12, 23) => Some("Mini fireball pickup"),
        _ => None,
    };
    match known {
        Some(name) => name.to_string(),
        None => format!("{class}/{model}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_size() {
        assert_eq!(
            Mc1Level::parse(&[0u8; 100]).err().unwrap(),
            Mc1LevelError::BadSize(100)
        );
    }

    #[test]
    fn parses_synthetic_level() {
        let mut data = vec![0u8; MC1_LEVEL_SIZE];
        // GEN_MAP: seed = 1921 at 0x04, raise = -1010 at 0x0C.
        data[0x04..0x08].copy_from_slice(&1921u32.to_le_bytes());
        data[0x0C..0x10].copy_from_slice(&(-1010i32).to_le_bytes());
        // One entity in slot 3: a Dragon (class 5, model 0) at (100, 200).
        let o = THINGS_OFFSET + 3 * THING_SIZE;
        data[o..o + 2].copy_from_slice(&5u16.to_le_bytes());
        data[o + 4..o + 6].copy_from_slice(&100u16.to_le_bytes());
        data[o + 6..o + 8].copy_from_slice(&200u16.to_le_bytes());
        // Wizard slot 1: aggression 200, tempo 128, accuracy 64, a
        // pre-granted allowed fireball.
        let w = WIZARDS_OFFSET + WIZARD_SIZE;
        data[w + 4..w + 6].copy_from_slice(&200u16.to_le_bytes());
        data[w + 8..w + 10].copy_from_slice(&128u16.to_le_bytes());
        data[w + 12..w + 14].copy_from_slice(&64u16.to_le_bytes());
        data[w + 16] = 1;
        data[w + 116] = 1;
        // Tail: 2 players, slot-1 starting castle level 3.
        data[FOOTER_OFFSET + 2..FOOTER_OFFSET + 4].copy_from_slice(&2u16.to_le_bytes());
        data[FOOTER_OFFSET + 5] = 3;

        let level = Mc1Level::parse(&data).unwrap();
        assert_eq!(level.gen_map.seed, 1921);
        assert_eq!(level.gen_map.raise, -1010);
        assert!(!level.reserved_nonzero);
        let active: Vec<_> = level.active_things().collect();
        assert_eq!(active.len(), 1);
        let (index, thing) = active[0];
        assert_eq!(index, 3);
        assert_eq!(
            (thing.class, thing.model, thing.x, thing.y),
            (5, 0, 100, 200)
        );
        assert_eq!(thing_name(thing.class, thing.model), "Dragon");
        assert_eq!(level.player_count, 2);
        assert_eq!(level.castle_levels, [0, 3, 0, 0, 0, 0, 0, 0]);
        let w1 = &level.wizards[1];
        assert_eq!((w1.aggression, w1.tempo, w1.accuracy), (200, 128, 64));
        assert_eq!(w1.pregrant[0], 1);
        assert_eq!(w1.allowed[0], 1);
        assert_eq!(level.wizards[0].pregrant, [0; 24]);
        // The decoded tail mirrors the raw footer bytes.
        assert_eq!(level.footer[1], 2);
        assert_eq!(level.footer[2], (3u16) << 8);
    }
}
