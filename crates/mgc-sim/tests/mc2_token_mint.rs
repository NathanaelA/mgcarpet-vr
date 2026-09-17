//! MC2 class-15 SPELL TOKENS as NATIVELY MINTED — the lanes the two
//! graded runners cannot see, because both seed the pool from the
//! recording (docs/CONFORMANCE.md "init-check"). Four laws, one per
//! test:
//!
//! - the level-start REIFY (`sub_55AB0`, Level.cpp:1313) mints each
//!   token at the CARPET's `position_0x4C_76`, not at the map origin;
//! - it does NOT arm the 64-tick re-steal lock `word_0x36_54` — that
//!   is `sub_68FF0`'s COLLECT store alone (EF:56076, NETHERW.EXE
//!   0x8D93C), so a fresh level's book is stealable from tick one;
//! - `SetDefaultSpells_5C0A0` (Spells.cpp:109) rewrites SPELLS row 23's
//!   `maxManaLimit_A` to 50000/70000/90000 before any level loads;
//! - `NewEvent_4A050` (Events.cpp:569) seeds @0x2A, not @0x2C, so a
//!   fresh jar's `word_0x2C_44` is 0, not the allocator's 100.
//!
//! Self-skips without baked mc2 data (game data is optional).

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::World;
use mgc_sim::ids::GameId;
use std::path::PathBuf;

#[path = "common/mod.rs"]
mod common;

fn baked_root() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    (p.join("mc2/level-000.mgcl").exists()
        && p.join("assets/mc2-night/build.tab.bin").exists()
        && !common::modded_bake(&p))
    .then_some(p)
}

/// A FRESH-BOOT MC2 world — the native constructor, no importer.
fn build_world(root: &std::path::Path, level: u32) -> Option<World> {
    let file = std::fs::File::open(root.join(format!("mc2/level-{level:03}.mgcl"))).unwrap();
    let pkg: mgc_formats::LevelPackage = mgc_formats::mgcl::read(file).unwrap();
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone().unwrap(),
        angle: terrain.angle.clone().unwrap(),
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).unwrap();
    let mut assets = FeatureAssets::parse(
        bundle.search.as_ref().unwrap(),
        bundle.build_tab.as_ref().unwrap(),
        bundle.build_dat.as_ref().unwrap(),
    )
    .unwrap()
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    if let Some(sp) = bundle.spells.as_deref() {
        assets = assets.with_spells(sp).unwrap();
    }
    if let Some(dims) = bundle.mc2_extent_dims(&root.join("assets")) {
        assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
    }
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_placeholders(true);
    w.set_mc2_night_shade(true);
    Some(w)
}

macro_rules! world {
    ($level:expr) => {{
        let Some(root) = baked_root() else {
            eprintln!("skipping: no baked data");
            return;
        };
        let Some(w) = build_world(&root, $level) else {
            eprintln!("skipping: the level has no terrain");
            return;
        };
        w
    }};
}

/// The level-start book is HANDED OVER, not PICKED UP. Retail's only
/// `word_0x36_54 = 64` is `sub_68FF0`'s collect block (EF:56076,
/// `NETHERW.EXE` 0x8D93C `66 c7 43 36 40 00`; a scan of the whole image
/// for `movw $imm16,0x36(reg)` finds no other 64). `sub_55AB0`'s reify
/// writes `parentId_0x28_40`, `byte[0] |= 1` and `SetSpell_6D5E0` and
/// nothing else, so the carried book is born with the lock CLEAR.
///
/// Player-visible: the lock's only reader on a token is
/// `World::mc2_spell_steal`'s gate (`sub_69300`, EF:55800), so the
/// pre-dig port opened every MC2 level with every book immune to an
/// m26 wraith's spell theft for 64 ticks.
#[test]
fn a_level_start_book_is_born_without_the_re_steal_lock() {
    let mut w = world!(0);
    w.mc2_grant_start_book(&[0, 1, 9]);
    let toks = w.debug_spell_tokens();
    assert!(!toks.is_empty(), "the ctor floor alone mints two tokens");
    for (slot, model, _own, lock, _tier, _upkeep, ..) in &toks {
        assert_eq!(
            *lock, 0,
            "slot {slot} (15,{model}): a REIFY does not arm the re-steal lock"
        );
    }
}

/// `sub_5C950` places player 0 at its own start marker before
/// `sub_55AB0` reifies the book, and the reify mints each token at
/// `&Entities[playerIndex]->position_0x4C_76` (Level.cpp:1319) — so
/// every level-start token sits ON the carpet, and is tile-linked
/// there. Witness: mc2l4 record 0, human carpet slot 265 and its 26
/// tokens (slots 266..291) all at (64896, 16000, 2256) = marker
/// (253, 62) at ground + 0x100.
#[test]
fn the_level_start_book_is_minted_at_the_carpet_start_marker() {
    let mut w = world!(0);
    w.mc2_grant_start_book(&[0, 1, 9]);
    let (mx, my) = w
        .debug_start_marker(0)
        .expect("level 000 authors a human start marker");
    let (x, y) = ((mx << 8).wrapping_add(128), (my << 8).wrapping_add(128));
    let z = w.ground_z_engine(x, y).wrapping_add(0x100);
    for (slot, model, _own, _lock, _tier, _upkeep, tx, ty, tz) in w.debug_spell_tokens() {
        assert_eq!(
            (tx, ty, tz),
            (x, y, z),
            "slot {slot} (15,{model}) must be born on the carpet, not at the map origin"
        );
    }
}

/// `SetDefaultSpells_5C0A0` (Spells.cpp:109) runs once over the freshly
/// loaded SPELLS table. Against the shipped `SPELLS.DAT` only its
/// `case 23:` arm moves a byte — `maxManaLimit_A` 300000/350000/400000
/// → 50000/70000/90000, shipped at `NETHERW.EXE` 0x80929/0x80930/
/// 0x80937 (`movl $0xc350,0x0a(%ebx)` / `$0x11170,0x24` / `$0x15f90,
/// 0x3e`). `SetSpell_6D5E0` copies the column into
/// `manaRegen_0x88_136`, the per-tick castle-pool upkeep gate.
/// Witness: mc2l4 record 0 slot 289, a tier-0 (15,23), `d88` 50000.
#[test]
fn spell_23_carries_the_set_default_spells_upkeep_column() {
    let mut w = world!(0);
    w.mc2_grant_start_book(&[23]);
    assert_eq!(
        w.debug_spell_mana_lanes(23).map(|l| l.0),
        Some(50_000),
        "row 23 tier 0's upkeep is the PATCHED 50000, not the DAT's 300000"
    );
}

/// MC2's `NewEvent_4A050` seeds `subSpellIndex_0x2A_42 = 100` and
/// leaves `word_0x2C_44` at the memset's 0 (Events.cpp:569 and :596,
/// both arms). The port's shared allocator writes `f44 = 100`, and on
/// class 15 `f44` IS @0x2C — so an authored GROUND JAR, which no adopt
/// ever scrubs, published a pending tier of 100. `sub_6D880` reads that
/// word as "tier 99" when a cast window expires.
#[test]
fn a_fresh_ground_jar_has_no_pending_tier() {
    let w = world!(5);
    let jars: Vec<_> = w
        .debug_spell_tokens()
        .into_iter()
        .filter(|t| t.2 == t.0 as u16) // unowned: @0x28 is still the self-tag
        .collect();
    assert!(!jars.is_empty(), "mc2:005 authors ground jars");
    for (slot, model, _own, _lock, tier, ..) in jars {
        assert_eq!(
            tier, 0,
            "slot {slot} (15,{model}): an unowned jar's @0x2C is retail's 0"
        );
    }
}

