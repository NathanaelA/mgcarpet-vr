//! WAVE 123 / DIG X1 — ⭐⭐⭐ THE KNOCKBACK BEARING IS TAKEN OFF THE
//! SOURCE RECORD WHETHER OR NOT IT IS STILL ALIVE.
//!
//! MC2's wizard-damage arm `sub_5EFA0` (EF:61012-61046) gates on the
//! LETTER, not on the sender:
//!
//! ```text
//!   v8 = a1x->str_0x5E_94.word_0x62_98;      // mail0.src
//!   if (v8) {
//!       ...
//!       v14  = a1x->str_0x5E_94.word_0x62_98;
//!       a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;
//!       v15x = Entities_EA3E4[v14];                       // EF:61037
//!       a1x->dword_0xA4_164x->yaw_0x1E_30 =
//!           Maths::sub_581E0_maybe_tan2(&v15x->position_0x4C_76,
//!                                       &a1x->position_0x4C_76);
//!       a1x->dword_0xA4_164x->moveBoost_0x1E_30 =
//!           a1x->str_0x5E_94.dword_0x5E_94 / 10;          // clamp 0..80
//!```
//!
//! `Entities_EA3E4[v14]` is a bare table index — there is NO
//! `class_0x3F_63` test in front of it. The proof that retail really
//! does dereference a freed record here is eight lines further down
//! its OWN arm: the flood-killer suppression reads
//! `v15x->class_0x3F_63 == 10 && v15x->model_0x40_64 == 67` off the
//! same pointer. A free clears the class byte and leaves
//! `position_0x4C_76` standing, so a projectile that expires on the
//! very tick its hit lands still supplies the bearing.
//!
//! The port had `&& self.g.ent[s].class64 != 0` on that lookup — an
//! INVENTED GUARD. It is silent except in exactly that case, and then
//! the register keeps the PREVIOUS hit's bearing and the whole death
//! fall flies the wrong way.
//!
//! ⭐ RECORDED WITNESS — `recordings/mc2l4.mgcr`, THE TAKE'S LAST
//! HEAD. Rival 292 is killed at t=8541 by slot 247, a (0,2) husk
//! already freed that tick but still parked at (31866, 60047). Retail
//! stamps bearing 1314 off it; the port kept the stale 385 from the
//! previous hit, and at t=8542 `sub_5D530`'s knock leg pushed the
//! corpse the wrong way: retail lands 292 at (31520, 60306), the port
//! at (31657, 60225) — and the grave (10,40), the revived (10,1) and
//! all four class-15 scatter tokens rode the same constant
//! (+137, -81) off. Fixing the guard takes mc2l4 from 2 segments /
//! 1 excess reset to ONE SEGMENT, BIT-EXACT over all 17,818 ticks.
//!
//! ⚠ NO FIXTURE CAN SEE THIS. The knock register is the port's
//! per-rival `Mc2Rival::{knock_dir, knock_mag}` (retail's
//! `yaw_0x1E_30`/`moveBoost_0x1E_30` in the player block), and the
//! conformance PAIR IMPORTER restores both at every boundary
//! (`mc2/rivals.rs`, the `ai.knock_dir`/`ai.knock_mag` seat). The
//! write is one tick upstream of the read, so the pair at head-1
//! (t=8541) is CLEAN with the law off and the head classifies
//! INHERITED — a head-1 fixture would be VACUOUS. Per
//! docs/CONFORMANCE.md that makes a unit test the correct lane.
//!
//! Kill switch: `MGC_NO_KNOCK_FROM_DEAD_SOURCE=1`.
//!
//! Self-skips without baked mc2 data (game data is optional).

use mgc_formats::LevelPackage;
use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
use mgc_sim::ids::GameId;
use mgc_sim::mc2::rivals::Mc2RivalConfig;
use std::path::Path;

fn load(level: &str) -> Option<World> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked");
    let root = root.as_path();
    let bundle = mgc_formats::bundle::Bundle::load(&root.join("assets/mc2-night")).ok()?;
    let assets = FeatureAssets::parse(
        bundle.search.as_ref()?,
        bundle.build_tab.as_ref()?,
        bundle.build_dat.as_ref()?,
    )
    .ok()?
    .with_bldgprm(bundle.bldgprm.as_deref().unwrap_or_default());
    let assets = match bundle.mc2_extent_dims(&root.join("assets")) {
        Some(dims) => assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims)),
        None => assets,
    };
    let assets = match bundle.spells.as_deref() {
        Some(sp) => assets.with_spells(sp).ok()?,
        None => assets,
    };
    let file = std::fs::File::open(root.join("mc2").join(format!("{level}.mgcl"))).ok()?;
    let pkg: LevelPackage = mgc_formats::mgcl::read(file).ok()?;
    let terrain = pkg.terrain.as_ref()?;
    let planes = Planes {
        height: terrain.height.clone(),
        tile_type: terrain.tile_type.clone(),
        shading: terrain.shading.clone()?,
        angle: terrain.angle.clone()?,
        ceiling: terrain.ceiling.clone().unwrap_or_default(),
    };
    let seed = pkg.gen_params.as_ref().map_or(0, |g| g.seed);
    let mut w = World::new_for_game(planes, &pkg.things.things, seed, assets, GameId::Mc2);
    w.set_mc2_night_shade(true);
    let (cfgs, count) = rival_configs(&pkg);
    w.set_mc2_wizards(&cfgs, count);
    Some(w)
}

fn rival_configs(pkg: &LevelPackage) -> ([Option<Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<Mc2RivalConfig>; 8] = Default::default();
    let (Some(wz), Some(h)) = (pkg.wizards.as_ref(), pkg.header.as_ref()) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in wz.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(reflexes), Some(perception)) = (cfg.reflexes, cfg.perception) else {
            continue;
        };
        let mut start = [false; 26];
        let mut start_level = [0u8; 26];
        let mut blocked = [false; 26];
        for s in 0..26 {
            start[s] = cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
            start_level[s] = cfg
                .starting_spell_levels
                .get(s)
                .copied()
                .unwrap_or(0)
                .min(2);
            blocked[s] = cfg.blocked_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(Mc2RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            perception: perception.clamp(0, 255) as u8,
            reflexes: reflexes.clamp(0, 255) as u8,
            life: cfg.life.unwrap_or(0).max(0) as u16,
            castle_level: h.players[slot].max(0) as u8,
            start,
            start_level,
            blocked,
        });
    }
    (out, count)
}


/// The first live class-15 manifestation token: a static, always-
/// present record parked away from the wizard, and harmless to free
/// for the one tick this test needs.
fn a_token(w: &World) -> Option<(usize, u16, u16)> {
    (0u8..26)
        .filter_map(|m| w.debug_flock_probe(15, m).into_iter().next())
        .map(|r| (r.slot, r.x, r.y))
        .find(|&(_, x, y)| (x, y) != (0, 0))
}

/// Deliver one lethal letter to a rival from `src`, optionally with
/// `src` FREED IN PLACE first, and report the knock register the
/// damage arm leaves behind.
fn knock_after_lethal_hit(free_the_source: bool) -> Option<(u16, i16)> {
    let mut w = load("level-004")?;
    let views = w.rival_views();
    assert!(!views.is_empty(), "level-004 spawns rivals");
    let color = views[0].slot;
    let idle = PlayerCommand::default();
    let pose = PlayerPose::from_tiles(8.0, 20.0, 8.0, 0.0, 0.0, 0.0);

    // Two settling ticks, then park the wizard well away from the
    // castle so the bearing is unambiguous.
    w.tick(pose, idle);
    w.tick(pose, idle);
    w.debug_place_mc2_rival(color, 40.0, 200.0)
        .expect("the rival has a live carpet record");
    assert_eq!(
        w.debug_mc2_rival_knock(color),
        Some((0, 0)),
        "the knock register is still unarmed before the hit"
    );

    let (src, _, _) = a_token(&w).expect("a class-15 manifestation to fire the letter from");
    if free_the_source {
        w.debug_mc2_free_in_place(src);
    }
    w.debug_hit_mc2_rival(color, src as u16, u32::MAX / 4);
    w.tick(pose, idle);
    w.debug_mc2_rival_knock(color)
}

/// ⭐⭐⭐ Freeing the source record must not change the knockback
/// bearing: retail indexes `Entities_EA3E4[]` with no liveness test,
/// and a freed record keeps its position.
///
/// With `MGC_NO_KNOCK_FROM_DEAD_SOURCE=1` (the pre-dig invented
/// `class64 != 0` guard) the freed-source arm leaves the register
/// untouched at `(0, 0)` and this test fails on both assertions.
#[test]
fn the_knockback_bearing_reads_a_freed_source_record() {
    let Some(alive) = knock_after_lethal_hit(false) else {
        eprintln!("skipping: no baked mc2 gamedata");
        return;
    };
    let dead = knock_after_lethal_hit(true).expect("the same world loads twice");

    // The letter is `u32::MAX / 4`, so the magnitude saturates the
    // EF:61045 clamp either way.
    assert_eq!(
        alive.1, 80,
        "a live source arms the impulse at the EF:61045 ceiling"
    );
    assert_ne!(
        alive.0, 0,
        "the bearing off a source 160 tiles away is not straight north"
    );
    assert_eq!(
        dead, alive,
        "freeing the source in place must not change the knock register \
         (retail: EF:61037 indexes the entity table with no class test)"
    );
}
