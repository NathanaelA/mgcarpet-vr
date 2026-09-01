//! ⭐⭐⭐ THE FOOL'S-MANA RETALIATION FIREBALL IS A STRANGER TO
//! EVERYONE — round 98, dig 98-Q18.
//!
//! `sub_36770` (the tier-0/1 FIREBALL retaliation) and `sub_36850`
//! (the tier-2/3 LIGHTNING retaliation) are the same eight statements
//! in the same order except for ONE: the lightning copies the sphere's
//! `id_0x1A_26` onto the flyer, and the fireball has NO `id_0x1A_26`
//! statement at all. Verified byte-for-byte in the shipped
//! `NETHERW.EXE` — `sub_36770` is file `0x5AF70..0x5B04C` in full and
//! its only stores on the new record `%ebx` are
//!   0x5AFA6 `mov %ax,0x96(%ebx)`   word_0x96_150 = the claim latch
//!   0x5AFB6 `mov %ax,0x1c(%ebx)`   yaw   = roll
//!   0x5AFBE `mov %ax,0x1e(%ebx)`   pitch = fov
//!   0x5AFCC `mov %dx,0x50(%ebx)`   z += the sphere's fov
//!   0x5AFD4 `mov %ax,0x2a(%ebx)`   subSpellIndex
//! — no `0x1a(%ebx)` anywhere, while `sub_36850` carries the pair
//!   0x5B093 `mov 0x1a(%eax),%ax` / 0x5B097 `mov %ax,0x1a(%ebx)`.
//! (EF:26671-96 and EF:26700-27.)
//!
//! So the fireball keeps `NewEvent_4A050`'s ctor seed
//! `id_0x1A_26 = <own pool index>` (Events.cpp:570) and belongs to
//! nobody. That matters because `id_0x1A_26` IS the engine's only
//! friendly-fire rule: `sub_10C80`'s ch0 area write gates its CASTLE
//! pre-pass on `iix->id_0x1A_26 != a1x->id_0x1A_26` alone (EF:4066),
//! and a castle's `sub_106C0` box is ~26 tiles wide — so retail's
//! retaliation fireball bills the Fool's-Mana CASTER'S OWN CASTLE from
//! twenty tiles away, and the port's (which had inherited the sphere's
//! fused `id24`, i.e. retail's `parentId_0x28_40` = that caster) billed
//! it nothing.
//!
//! No pair fixture can assert this: `EntObsMc2` carries `owner` (@0x28)
//! but no `@0x1A` lane at all, and `import_ent_mc2` re-seeds `id24`
//! from `@0x28`-else-`@0x1A` on every pair — so the pair census is
//! byte-identical with the law on and off. Unit test per
//! docs/CONFORMANCE.md.
//!
//! Non-vacuous: FAILS with `MGC_NO_FOOLS_BOLT_STRANGER=1`.

use mgc_sim::engine::features::{FeatureAssets, Planes};
use mgc_sim::engine::world::{PlayerCommand, PlayerPose, World};
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

fn build_world(root: &std::path::Path) -> Option<World> {
    let file = std::fs::File::open(root.join("mc2/level-000.mgcl")).unwrap();
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

fn open_spot(w: &World) -> (u16, u16) {
    let p = w.planes();
    for cy in (24..222u16).step_by(3) {
        for cx in (24..232u16).step_by(3) {
            let t = (cy as usize % 256) * 256 + (cx as usize % 256);
            if p.angle[t] & 0x80 == 0 && p.angle[t] & 0xF != 0 {
                return (cx, cy);
            }
        }
    }
    panic!("no open spot on the level");
}

fn pose_at(w: &World, cx: u16, cy: u16) -> PlayerPose {
    let (px, pz) = (cx as f32 + 0.5, cy as f32 + 0.5);
    let alt = w.ground_height_tiles(px, pz) + 2.0;
    PlayerPose::from_tiles(px, alt, pz, 0.0, 0.0, 0.0)
}

/// (slot, id24) of every class-9 model-0 record in the pool.
fn bolts(w: &World) -> Vec<(usize, u16)> {
    w.debug_pool()
        .1
        .iter()
        .filter(|e| e.class == 9 && e.model == 0)
        .map(|e| (e.slot, e.id24))
        .collect()
}

#[test]
fn the_fools_mana_retaliation_fireball_owns_itself() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    // An AUTHORED ground sphere is a live TIER-0 trap for any
    // non-owner (`sub_36680`'s only no-trap arm is
    // `parentId == claimer`, EF:26623) — the same arm the cast decoy
    // takes, and the one the mc2l6-rsg witness ran through.
    let sphere = w.debug_mc2_spawn_ground_sphere((cx << 8) | 128, (cy << 8) | 128);
    assert!(sphere != 0, "the authored ground sphere spawned");

    let before: Vec<usize> = bolts(&w).into_iter().map(|(s, _)| s).collect();
    w.debug_mc2_claim_sphere_at(sphere, 12345);
    w.tick(pose, PlayerCommand::default());

    let fresh: Vec<(usize, u16)> = bolts(&w)
        .into_iter()
        .filter(|(s, _)| !before.contains(s))
        .collect();
    assert_eq!(
        fresh.len(),
        1,
        "the sprung tier-0 trap fires exactly one (9,0) fireball"
    );
    let (slot, id24) = fresh[0];
    assert_eq!(
        id24, slot as u16,
        "`sub_36770` writes no id_0x1A_26 (NETHERW.EXE 0x5AF70-0x5B04C), so the \
         retaliation fireball keeps NewEvent_4A050's own-slot seed and is a \
         stranger to the sphere's owner — otherwise sub_10C80's ch0 castle \
         pre-pass (EF:4066) spares that owner's castle. got id24 {id24} on slot {slot}"
    );
    assert_ne!(
        id24, sphere as u16,
        "…and in particular it must NOT inherit the sphere's own id (which is \
         what the LIGHTNING sibling sub_36850 does, EXE 0x5B093-0x5B097)"
    );
}
