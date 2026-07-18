//! Retail-format campaign save files, both games — the durable
//! progression records (docs/traces/mc1-campaign-save-menu.md,
//! mc2-campaign-save-menu.md). NOT the mid-level world snapshots
//! (retail MC1 `gam%05d.dat` / MC2 `SLEV*` dump the native RAM
//! layout; out of scope by design — the campaign save captures
//! everything between levels, which is also where retail MC2 lets
//! you save from the menu).
//!
//! Layouts are byte-exact with retail so GOG-era saves load and our
//! saves load back into retail. Per-game directories (the formats
//! collide — both use `.gam`, different slot counts):
//! `saves/mc1/carpddNN.gam` (6 slots, NN = 00..05),
//! `saves/mc1hw/carpddNN.gam` (byte-identical format),
//! `saves/mc2/SAVEn.GAM` (8 slots, n = 1..8).

use std::path::{Path, PathBuf};

/// The per-game saves directory, next to `mgcarpet.json` (never
/// inside `gamedata/`).
pub fn dir(tag: &str) -> PathBuf {
    Path::new("saves").join(tag)
}

// ---------------------------------------------------------------- MC1

/// MC1/HW slot count (`off_96864[6][21]`, remc1 :4228).
pub const MC1_SLOTS: usize = 6;
/// MC1 record size: magic 4 + name 20 + two 32-byte config buffers +
/// 12 settings + 4 level + 24 blob + 2 counters + 12 settings again.
pub const MC1_SIZE: usize = 142;

/// `save/carpdd%02X.gam` (remc1 :61982) — slot is 0-based.
pub fn mc1_path(tag: &str, slot: usize) -> PathBuf {
    dir(tag).join(format!("carpdd{slot:02x}.gam"))
}

/// The MC1/HW menu save (`sub_51C90_51FD0` write / `sub_51AF0_51E30`
/// read, remc1 :62052-62081 / :62007-62041): pure campaign
/// progression. Fields we don't model are carried opaquely so a
/// load→save round trip preserves retail bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mc1Save {
    /// Slot label (20 bytes on disk, NUL-padded).
    pub name: String,
    /// Config `var_u8_29` (Basic.h:600): `[0]` = use-custom-loadout
    /// flag; carried opaquely.
    pub buf29: [u8; 32],
    /// Config `var_u8_61` (Basic.h:601): the 12-byte custom
    /// starting-spell loadout buffer; carried opaquely.
    pub buf61: [u8; 32],
    /// World struct +8597 settings block (written twice in retail).
    pub settings: [u8; 12],
    /// Current campaign level (`var_u16_17`), decoded from the
    /// obfuscated `4*(level + name_counter + player_count)` word
    /// (remc1 :62036).
    pub level: u16,
    /// World struct +15318 = `var_15318_1995_892[24]` — the PERSISTENT
    /// collected-spell flags, one byte per spell (the campaign memory;
    /// remc1 :49148 copy-in, ROADMAP "Campaign spell progression").
    /// Level-start grant = these flags ∩ the level's availability mask.
    pub blob24: [u8; 24],
    /// `byte_12CBD0` — the "CARPET%d" name-rotation counter (0-9).
    pub name_counter: u8,
    /// `byte_12CBD1` — the world-map player-count pick.
    pub player_count: u8,
    /// The second +8597 copy. Retail writes the block twice; kept
    /// separately so foreign saves round-trip even if they differ.
    pub settings_b: [u8; 12],
}

impl Mc1Save {
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() < MC1_SIZE {
            return Err(format!("MC1 save: {} bytes (need {MC1_SIZE})", b.len()));
        }
        let magic = u32::from_le_bytes(b[0..4].try_into().unwrap());
        if magic != 4 {
            return Err(format!("MC1 save: bad magic {magic} (want 4)"));
        }
        let mut s = Self {
            name: cstr(&b[4..24]),
            ..Self::default()
        };
        s.buf29.copy_from_slice(&b[24..56]);
        s.buf61.copy_from_slice(&b[56..88]);
        s.settings.copy_from_slice(&b[88..100]);
        let enc = u32::from_le_bytes(b[100..104].try_into().unwrap());
        s.blob24.copy_from_slice(&b[104..128]);
        s.name_counter = b[128];
        s.player_count = b[129];
        s.settings_b.copy_from_slice(&b[130..142]);
        // level = enc/4 − counters (remc1 :62036); clamp rather than
        // reject — a corrupt word yields level 0, not a load failure.
        s.level = (enc / 4)
            .saturating_sub(s.name_counter as u32 + s.player_count as u32)
            .min(u16::MAX as u32) as u16;
        Ok(s)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(MC1_SIZE);
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&fixed::<20>(&self.name));
        b.extend_from_slice(&self.buf29);
        b.extend_from_slice(&self.buf61);
        b.extend_from_slice(&self.settings);
        let enc = 4 * (self.level as u32 + self.name_counter as u32 + self.player_count as u32);
        b.extend_from_slice(&enc.to_le_bytes());
        b.extend_from_slice(&self.blob24);
        b.push(self.name_counter);
        b.push(self.player_count);
        b.extend_from_slice(&self.settings_b);
        debug_assert_eq!(b.len(), MC1_SIZE);
        b
    }
}

// ---------------------------------------------------------------- MC2

/// MC2 slot count (`SAVE%d.GAM` 1..=8, port_filesystem.cpp:549).
pub const MC2_SLOTS: usize = 8;
/// MC2 record size (write order MenusAndIntros.cpp:2606-2616).
pub const MC2_SIZE: usize = 1319;
/// The `.GAM` signature word (`0xFFFFFFF7` = -9).
pub const MC2_MAGIC: u32 = 0xFFFF_FFF7;

/// `SAVE/SAVE%d.GAM` — slot is 0-based here, retail names 1-based.
pub fn mc2_path(slot: usize) -> PathBuf {
    dir("mc2").join(format!("SAVE{}.GAM", slot + 1))
}

/// One secret-portal record as saved (17 bytes,
/// Type_SecretMapScreenPortals_E2970.h — packed, all LE):
/// time i32 @0, parent main level u16 @4, level number u16 @6,
/// map pos u16 @8/@10, activated u16 @12 (3=hidden, 2=revealed,
/// 1=completed), sprite u16 @14, byte @16.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecretPortal {
    pub time: i32,
    pub parent: u16,
    pub level: u16,
    pub pos: (u16, u16),
    pub activated: u16,
    pub sprite: u16,
    pub byte16: u8,
}

impl SecretPortal {
    fn decode(b: &[u8; 17]) -> Self {
        let u16le = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        Self {
            time: i32::from_le_bytes(b[0..4].try_into().unwrap()),
            parent: u16le(4),
            level: u16le(6),
            pos: (u16le(8), u16le(10)),
            activated: u16le(12),
            sprite: u16le(14),
            byte16: b[16],
        }
    }

    fn encode(&self) -> [u8; 17] {
        let mut b = [0u8; 17];
        b[0..4].copy_from_slice(&self.time.to_le_bytes());
        b[4..6].copy_from_slice(&self.parent.to_le_bytes());
        b[6..8].copy_from_slice(&self.level.to_le_bytes());
        b[8..10].copy_from_slice(&self.pos.0.to_le_bytes());
        b[10..12].copy_from_slice(&self.pos.1.to_le_bytes());
        b[12..14].copy_from_slice(&self.activated.to_le_bytes());
        b[14..16].copy_from_slice(&self.sprite.to_le_bytes());
        b[16] = self.byte16;
        b
    }
}

/// The MC2 campaign save (`SaveGameDialog` write MenusAndIntros.cpp:
/// 2606-2616, `LoadGameDialog` read :1493-1517): pure progression +
/// player stats, no world snapshot. Main-portal states are NOT
/// stored — retail reconstructs them from `levels_completed` (the
/// first N portals activate, :1521-1544), and so do we.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mc2Save {
    /// Editable slot label (20 bytes).
    pub label: String,
    /// `player_name_57ar` (32 bytes).
    pub player_name: String,
    /// `savestring_89` (32 bytes, opaque carry).
    pub savestring: [u8; 32],
    /// The secret-portal table (6 entries; [5] is the terminator).
    pub secrets: [SecretPortal; 6],
    /// `m_GameSettings` (16 bytes, opaque carry — retail display/
    /// gameplay toggles; our settings live in mgcarpet.json).
    pub game_settings: [u8; 16],
    /// Count of activated main portals (`numLevelsCompleted`,
    /// :2599-2604) — THE main-campaign progression record.
    pub levels_completed: u32,
    /// The current level's complete-flags word (`byte[2]` of
    /// `dw_w_b_0_2BDE_11230`: bit 1 advance, bit 3 reload, bit 4
    /// secret-route — docs/traces/mc2-campaign-save-menu.md).
    pub level_flags: u32,
    /// `str_611` — the 505-byte player spell/XP/mana block. Opaque
    /// until the layout lands; the campaign driver keeps its own
    /// book-carry alongside and syncs what it knows.
    pub str611: [u8; 505],
    /// Per-main-level score stats, 25×5 (`x_DWORD_17DBC8x`).
    pub main_stats: [[i32; 5]; 25],
    /// Per-secret-level score stats, 5×5 (`x_DWORD_17DDBCx`;
    /// sub_82AB0 EF:47027-33).
    pub secret_stats: [[i32; 5]; 5],
}

impl Default for Mc2Save {
    fn default() -> Self {
        Self {
            label: String::new(),
            player_name: String::new(),
            savestring: [0; 32],
            secrets: crate::campaign::mc2_secret_portals_pristine(),
            game_settings: [0; 16],
            levels_completed: 0,
            level_flags: 0,
            str611: [0; 505],
            main_stats: [[0; 5]; 25],
            secret_stats: [[0; 5]; 5],
        }
    }
}

impl Mc2Save {
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() < MC2_SIZE {
            return Err(format!("MC2 save: {} bytes (need {MC2_SIZE})", b.len()));
        }
        let magic = u32::from_le_bytes(b[0..4].try_into().unwrap());
        if magic != MC2_MAGIC {
            return Err(format!("MC2 save: bad signature {magic:#010x}"));
        }
        let mut s = Self {
            label: cstr(&b[4..24]),
            player_name: cstr(&b[24..56]),
            ..Self::default()
        };
        s.savestring.copy_from_slice(&b[56..88]);
        for (i, p) in s.secrets.iter_mut().enumerate() {
            let o = 88 + i * 17;
            *p = SecretPortal::decode(b[o..o + 17].try_into().unwrap());
        }
        s.game_settings.copy_from_slice(&b[190..206]);
        s.levels_completed = u32::from_le_bytes(b[206..210].try_into().unwrap());
        s.level_flags = u32::from_le_bytes(b[210..214].try_into().unwrap());
        s.str611.copy_from_slice(&b[214..719]);
        for (l, row) in s.main_stats.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                let o = 719 + (l * 5 + k) * 4;
                *v = i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            }
        }
        for (l, row) in s.secret_stats.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                let o = 1219 + (l * 5 + k) * 4;
                *v = i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            }
        }
        Ok(s)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(MC2_SIZE);
        b.extend_from_slice(&MC2_MAGIC.to_le_bytes());
        b.extend_from_slice(&fixed::<20>(&self.label));
        b.extend_from_slice(&fixed::<32>(&self.player_name));
        b.extend_from_slice(&self.savestring);
        for p in &self.secrets {
            b.extend_from_slice(&p.encode());
        }
        b.extend_from_slice(&self.game_settings);
        b.extend_from_slice(&self.levels_completed.to_le_bytes());
        b.extend_from_slice(&self.level_flags.to_le_bytes());
        b.extend_from_slice(&self.str611);
        for row in &self.main_stats {
            for v in row {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        for row in &self.secret_stats {
            for v in row {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        debug_assert_eq!(b.len(), MC2_SIZE);
        b
    }
}

/// `str_611` internal offsets, relative to the 505-byte block
/// (global_types.h:174-216 — 10 parallel per-spell arrays + scalars;
/// docs/traces/mc2-campaign-save-menu.md). Only the progression
/// fields are typed; the rest (cooldowns, UI slot order, cursor
/// state) are re-derived by retail at level start and carried
/// opaquely here for byte-exact interop.
const S611_BANK: usize = 0; // i32[26] SpellExperience (banked XP)
const S611_VOL: usize = 104; // i32[26] spellsExperience (volatile XP)
const S611_ENABLED: usize = 208; // i16[26] manifestation handle (0 = not learned)
const S611_OWNED: usize = 390; // u8[26] granted/owned flag (THE learned set)
const S611_LEVELS: usize = 442; // u8[26] derived tier 0..2
const S611_SEL: usize = 468; // u8[26] selected sub-spell/tier
const S611_LEFT: usize = 494; // i16 left quick-slot spell (−1 = none)
const S611_RIGHT: usize = 496; // i16 right quick-slot spell

/// The typed MC2 progression view over `str_611` — exactly the
/// fields retail's own campaign carry copies between levels
/// (`sub_549A0` Level.cpp:1259-66) plus the hand bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mc2Book {
    pub owned: [bool; 26],
    /// Total XP per spell (banked + volatile — the tier-derivation
    /// law sums both, EF:43872-916).
    pub xp: [i32; 26],
    pub levels: [u8; 26],
    pub sel: [u8; 26],
    pub left: i8,
    pub right: i8,
}

impl Mc2Save {
    /// Read the progression book out of `str_611`.
    pub fn book(&self) -> Mc2Book {
        let b = &self.str611;
        let i16at = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
        let mut out = Mc2Book {
            left: i16at(S611_LEFT).clamp(-1, 25) as i8,
            right: i16at(S611_RIGHT).clamp(-1, 25) as i8,
            ..Default::default()
        };
        for s in 0..26 {
            let i32at = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            let bank = i32at(S611_BANK + s * 4);
            let vol = i32at(S611_VOL + s * 4);
            let enabled =
                u16::from_le_bytes([b[S611_ENABLED + s * 2], b[S611_ENABLED + s * 2 + 1]]);
            out.owned[s] = b[S611_OWNED + s] != 0 || enabled != 0;
            out.xp[s] = bank.saturating_add(vol);
            out.levels[s] = b[S611_LEVELS + s];
            out.sel[s] = b[S611_SEL + s];
        }
        out
    }

    /// Write the progression book into `str_611` (all XP banked,
    /// volatile zeroed — the between-levels shape; the manifestation
    /// handle is stamped 1 for learned spells since retail re-derives
    /// real handles at level start, EF:38650-779).
    pub fn set_book(&mut self, book: &Mc2Book) {
        let b = &mut self.str611;
        for s in 0..26 {
            b[S611_BANK + s * 4..S611_BANK + s * 4 + 4].copy_from_slice(&book.xp[s].to_le_bytes());
            b[S611_VOL + s * 4..S611_VOL + s * 4 + 4].copy_from_slice(&0i32.to_le_bytes());
            let handle: i16 = if book.owned[s] { 1 } else { 0 };
            b[S611_ENABLED + s * 2..S611_ENABLED + s * 2 + 2]
                .copy_from_slice(&handle.to_le_bytes());
            b[S611_OWNED + s] = book.owned[s] as u8;
            b[S611_LEVELS + s] = book.levels[s];
            b[S611_SEL + s] = book.sel[s];
        }
        b[S611_LEFT..S611_LEFT + 2].copy_from_slice(&(book.left as i16).to_le_bytes());
        b[S611_RIGHT..S611_RIGHT + 2].copy_from_slice(&(book.right as i16).to_le_bytes());
    }
}

// ------------------------------------------------------------- helpers

/// NUL-terminated fixed field → String (lossy — retail names are
/// plain ASCII).
fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// String → NUL-padded fixed field, truncated to N−1 so the
/// terminator always fits (retail readers expect one).
fn fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut b = [0u8; N];
    for (i, &c) in s.as_bytes().iter().take(N - 1).enumerate() {
        b[i] = c;
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mc1_round_trip() {
        let mut s = Mc1Save {
            name: "WIZARD".into(),
            level: 23,
            name_counter: 3,
            player_count: 2,
            ..Default::default()
        };
        s.buf29[0] = 1;
        s.buf61[..4].copy_from_slice(&[9, 8, 7, 6]);
        s.blob24[23] = 0xAB;
        let bytes = s.encode();
        assert_eq!(bytes.len(), MC1_SIZE);
        // The obfuscated level word: 4*(23+3+2) = 112.
        assert_eq!(&bytes[100..104], &112u32.to_le_bytes());
        assert_eq!(Mc1Save::decode(&bytes).unwrap(), s);
    }

    #[test]
    fn mc1_rejects_garbage() {
        assert!(Mc1Save::decode(&[0u8; MC1_SIZE]).is_err()); // magic 0
        assert!(Mc1Save::decode(&[4, 0, 0, 0]).is_err()); // short
    }

    #[test]
    fn mc2_round_trip() {
        let mut s = Mc2Save {
            label: "MY SAVE".into(),
            player_name: "VISSILUTH".into(),
            levels_completed: 7,
            level_flags: 0x0A,
            ..Default::default()
        };
        // Secret 31 revealed (parent 7 just completed).
        s.secrets[1].activated = 2;
        s.secrets[1].sprite = 270;
        s.str611[0] = 0x11;
        s.str611[504] = 0x99;
        s.main_stats[6] = [100, 80, 90, 75, 12345];
        s.secret_stats[4][0] = -1;
        let bytes = s.encode();
        assert_eq!(bytes.len(), MC2_SIZE);
        assert_eq!(&bytes[0..4], &[0xF7, 0xFF, 0xFF, 0xFF]);
        // The load path's readbuffer+12 activated probe lands on our
        // entry 1's activated word: offset 88 + 17 + 12.
        assert_eq!(bytes[88 + 17 + 12], 2);
        assert_eq!(Mc2Save::decode(&bytes).unwrap(), s);
    }

    #[test]
    fn mc2_book_round_trips_through_str611() {
        let mut save = Mc2Save::default();
        let mut book = Mc2Book {
            left: 0,
            right: 4,
            ..Default::default()
        };
        book.owned[0] = true; // fireball
        book.owned[4] = true;
        book.xp[4] = 12345;
        book.levels[4] = 2;
        book.sel[4] = 1;
        save.set_book(&book);
        assert_eq!(save.book(), book);
        // The typed view survives the byte codec too.
        let loaded = Mc2Save::decode(&save.encode()).unwrap();
        assert_eq!(loaded.book(), book);
        // Learned spells carry a nonzero manifestation handle for
        // retail's enabled-gate; XP lands banked.
        assert_eq!(loaded.str611[S611_OWNED + 4], 1);
        assert_eq!(
            &loaded.str611[S611_BANK + 16..S611_BANK + 20],
            &12345i32.to_le_bytes()
        );
    }

    #[test]
    fn mc2_rejects_garbage() {
        assert!(Mc2Save::decode(&[0u8; MC2_SIZE]).is_err());
    }

    #[test]
    fn slot_paths() {
        assert_eq!(mc1_path("mc1hw", 0), Path::new("saves/mc1hw/carpdd00.gam"));
        assert_eq!(mc2_path(0), Path::new("saves/mc2/SAVE1.GAM"));
        assert_eq!(mc2_path(7), Path::new("saves/mc2/SAVE8.GAM"));
    }
}
