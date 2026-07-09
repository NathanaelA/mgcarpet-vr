//! Game identity + the minimal unified ID registries (ROADMAP
//! "MULTI-GAME ARCHITECTURE", Phase 2).
//!
//! **The keying rule**: local ID spaces are PER GAME — MC1 spell 5
//! is not MC2 spell 5, MC1 creature model 7 is not MC2 model 7. Any
//! data structure that crosses games (the authenticity matrix's
//! cross-imports, replay headers, cross-game option registries) must
//! key by `(GameId, local id)`, never by the bare local id. Nothing
//! crosses games yet; the rule is recorded here so the first thing
//! that does gets the key right.
//!
//! [`GameId`] is the sim-side identity; the package side's
//! [`mgc_formats::Game`] converts into it (`From`), keeping the
//! sim's per-game selection (chassis/verbs/registry methods) out of
//! the formats crate.

use crate::chassis::ChassisParams;
use crate::verbs::VerbSet;

/// Which game's rules a world runs. Hidden Worlds is retail's own
/// sibling binary of the MC1 engine — same chassis, same verb
/// column, its (eventual) deviations land as a small override set,
/// not a third column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameId {
    #[default]
    Mc1,
    Mc1Hw,
    Mc2,
}

impl GameId {
    /// The game's pristine chassis constants (tier 1).
    pub const fn chassis(self) -> ChassisParams {
        match self {
            GameId::Mc1 | GameId::Mc1Hw => ChassisParams::MC1,
            GameId::Mc2 => ChassisParams::MC2,
        }
    }

    /// The game's pristine tier-5 verb column.
    pub const fn verbs(self) -> VerbSet {
        match self {
            GameId::Mc1 | GameId::Mc1Hw => VerbSet::MC1,
            GameId::Mc2 => VerbSet::MC2,
        }
    }

    /// The THING registry: does this game's SERVING spawn column know
    /// `(class, model)`? Unknown things degrade gracefully at the
    /// seam — a misfit note plus (optionally) a placeholder billboard,
    /// never a crash. The MC2 set is the PORTED creator set (Phase-3
    /// slice) and grows entry by entry; everything MC2-authored
    /// outside it is a visible misfit, by design.
    ///
    /// MENTAL MODEL (Phase-3 review): the key is a known CREATOR
    /// ENTRY, not a known creature — a creator may produce an entity
    /// of a DIFFERENT class than authored (MC2's (5,19) ctor spawns a
    /// class-9 flyer, remc2 :34882), so the ledger describes disk
    /// records, not runtime entities. HW currently shares the MC1
    /// column unconditionally; its delta pass (Phase 4) will need a
    /// per-game override point here.
    pub fn known_thing(self, class: u16, model: u16) -> bool {
        match self {
            GameId::Mc1 | GameId::Mc1Hw => crate::mc1::known_thing(class, model),
            GameId::Mc2 => matches!(
                (class, model),
                // Class-5 wave A + the multipart subsystem + the
                // (5,10) doomsday pyramid (Phase 4.3, docs/traces/):
                // every creature except 5..=8 | 11 (non-spawnable
                // stubs). 15 is never authored but IS runtime-
                // spawned: the castle guard respawn (EF:61488).
                (5, 0..=4 | 9 | 10 | 12..=28)
                | (9, 13)
                // Class-3 models 4..=11 are the 8 wizard start-
                // position markers (sub_4A820.. EF:33259 — writes
                // array_0x2362[N], spawns nothing). Known
                // non-entities, not misfits.
                | (3, 4..=11)
                | (10, 45)
                // Class-11 switches (remc2 AddSwitchXX_50A90 :37059
                // + the strB0 tick table): 0/1 = enter/leave
                // one-shot, 2/3 = enter/leave repeating, 4 =
                // level-end release, 12/31 = X-markers, 13..=30 +
                // 33..=44 = the slot-condition band, 32 =
                // stage-gated release. Models 5..=11 stay misfits
                // (handlers OPEN in the trace).
                | (11, 0..=4 | 12..=44)
                // Scenery: tree/stone/dolmen + the Phase-4.3 band
                // (statics 3-5, cave bee 6, falling props 7/8).
                | (2, 0..=8)
                // Ground fire + the "Big explosion" route marker
                // (NewAdd0A00_4E320 / NewAdd0A01_4E3B0, :35332-73)
                // + the (10,6) standing ground fire (NewAdd0A06,
                // docs/traces/mc2-class10-m6-m9-m11-m28-m31.md §1).
                | (10, 0..=1 | 6)
                // The MC2 teleporter pad (sub_4FE40; par1/par2 = the
                // warp destination tile —
                // docs/traces/mc2-class10-m50-chains-and-tail.md §2).
                | (10, 34)
                // The (10,50) ridge-fence chain + its (10,51)
                // traveling beam (sub_49090 → sub_48880; same doc
                // §1) — chained records are load-consumed, the rest
                // degrade to the one-tick marker / one-stamp beam.
                | (10, 50 | 51)
                // The (10,28) road + (10,31) river terrain-authoring
                // chains (sub_48400 staircase / sub_487D0 — the
                // river carve is retail-inert;
                // docs/traces/mc2-terrain-author-painters.md §1/§3).
                | (10, 28 | 31)
                // The tail-effect band (mc2::tail,
                // mc2-class10-m50-chains-and-tail.md): 8 = the dead
                // creator (a known no-spawn), 11 = the fire spray
                // (model-19 remap), 15 fire trail, 17 meteor, 23/25
                // blasts, 52 castle anchor, 54 aura.
                | (10, 8 | 11 | 15 | 17 | 22 | 23 | 25 | 52 | 54 | 71 | 76)
                // The (10,9) raise-land / apocalypse dome
                // (mc2::morph, mc2-class10-m9-dome-geometry.md).
                | (10, 9)
                // The class-10 effects band: the (10,5) splash, the
                // smoke-column family ((10,13)/(10,14) particles +
                // the (10,59)/(10,60) "quest point" emitters), and
                // the (10,29) one-tick stage marker / waypoint-chain
                // record (docs/traces/mc2-class10-m59-m60.md +
                // mc2-class10-m29-m5-m13.md).
                | (10, 5 | 13 | 14 | 29 | 59 | 60 | 87)
                // The (14,1) riser's lower/raise triggers (ctors
                // sub_4F900/sub_4F950, actions 0x44/0x45 —
                // docs/traces/mc2-class14-m1-riser.md §6).
                | (10, 63 | 64)
                // The authored ground mana economy: 512-mana spheres
                // + the 2560 variant (CreateManaSphere512/2560,
                // EF:36595/:36601; both create model 39).
                | (10, 39 | 58)
                // Class-14 special map objects (X/end markers, the
                // pickup scroll, the terrain risers).
                | (14, 0..=5)
                // Class-15 spell tokens — the jars, one shared ctor
                // for all 26 spells (mc2::tokens,
                // docs/traces/mc2-class15-spell-tokens.md).
                | (15, 0..=25)
            ),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            GameId::Mc1 => "mc1",
            GameId::Mc1Hw => "mc1hw",
            GameId::Mc2 => "mc2",
        }
    }
}

impl From<mgc_formats::Game> for GameId {
    fn from(g: mgc_formats::Game) -> Self {
        match g {
            mgc_formats::Game::MagicCarpet1 => GameId::Mc1,
            mgc_formats::Game::HiddenWorlds => GameId::Mc1Hw,
            mgc_formats::Game::MagicCarpet2 => GameId::Mc2,
        }
    }
}
