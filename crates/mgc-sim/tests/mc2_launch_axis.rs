//! ROUND 98 — the CAST LAUNCH's two ungraded state lanes, both found
//! by `dump-state --port` on mc2l22 t=495 slot 719 (the basic
//! possession bolt the human casts at t=494→495) and both confirmed
//! at scale by `MGC_RAW_SHADOW=1` over the take's first 1200 pairs.
//!
//! 1. **`axis_0x9A_154x` — THE LAUNCH AIM POINT.** Every human cast
//!    thunk but Fool's Mana stamps the spawned flyer's @0x9A vector
//!    with the CASTER's own position stepped along the launch
//!    bearing: `sub_693F0` EF:55871-77 / `sub_69640` EF:55976-81 /
//!    `sub_6C170` EF:57673-77 / `sub_6CD20` EF:58077-81 /
//!    `sub_6B3E0` EF:57214-18 at 0x4000, `sub_69900` EF:56059-64 at
//!    **10240**, `sub_6CAC0` EF:57996-98 at 4096/pitch-0 + a
//!    terrain-alt z, `sub_6B610` EF:57310 at 10240. The port carried
//!    exactly ONE of the eight (the duel dart); the possession arms
//!    left the lane at zero. mc2l22 raw shadow: `(9,1)` dest_x/y/z
//!    121 rows EACH across 76 slots, `(9,17)` 3 rows each.
//!
//! 2. **`subSpellIndex_0x2A_42` — AN ABSENCE IN AN ENUMERATED LIST.**
//!    `sub_69900` (EF:56039-70), the leveled possession block in
//!    `sub_69640` (EF:55950-84) and the summon army `sub_6C170`
//!    (EF:57637-77) contain NO `subSpellIndex_0x2A_42` write at all,
//!    so their flyers keep `NewEvent_4A050`'s ctor default **100**
//!    (Events.cpp:569). The port's shared `mc2_launch` stamped the
//!    tier payload on every arm. mc2l22 raw shadow: `(9,1) f2a` 121
//!    rows retail 100 vs port 10, `(9,17) f2a` 3 rows retail 100 vs
//!    port 20.
//!
//! Both lanes are OUTSIDE `EntObsMc2`, so no conformance FIXTURE can
//! assert them (docs/CONFORMANCE.md: a law only free-run/shadow
//! visible belongs in a unit test). Kill switches:
//! `MGC_NO_MC2_LAUNCH_AXIS=1` and `MGC_NO_MC2_LAUNCH_2A_ABSENCE=1`.
//!
//! Self-skips without baked mc2 data (game data is optional).

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

/// A dry, unprotected tile to fly over.
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

fn lane(w: &World, slot: u16, name: &str) -> Option<i64> {
    w.port_ent_lanes_mc2(slot, 0, false)
        .unwrap_or_else(|| panic!("slot {slot} has no lanes"))
        .into_iter()
        .find(|(k, _)| *k == name)
        .unwrap_or_else(|| panic!("no lane {name}"))
        .1
}

/// Signed wrap distance on the 16-bit world axis.
fn wrap_d(a: u16, b: u16) -> i32 {
    (a.wrapping_sub(b) as i16) as i32
}

#[test]
fn the_basic_possession_bolt_carries_the_launch_aim_point_and_the_ctor_payload() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    // Possession tier 0 = the BASIC bolt, `sub_69900`'s (9,1).
    w.mc2_select_spell(1, 0, 0);
    w.tick(
        pose,
        PlayerCommand {
            fire_left: true,
            ..Default::default()
        },
    );
    let slot = w
        .sprite_lane()
        .into_iter()
        .find(|(_, _, c, m)| *c == 9 && *m == 1)
        .map(|t| t.0)
        .expect("the basic possession bolt (9,1) was launched");

    // ── LAW 2: no `subSpellIndex_0x2A_42` write on this arm ────────
    // `NewEvent_4A050`'s ctor default survives. Under the old
    // unconditional `mc2_launch` stamp this reads the tier payload
    // (mc2l22 records port 10 against retail's 100).
    assert_eq!(
        lane(&w, slot, "f2a"),
        Some(100),
        "sub_69900 (EF:56039-70) has no `subSpellIndex_0x2A_42 =` \
         statement, so the bolt keeps NewEvent_4A050's ctor 100 \
         (Events.cpp:569)"
    );

    // ── LAW 1: the launch aim point ────────────────────────────────
    // `v2x->axis_0x9A_154x = a2x->position_0x4C_76;` then
    // `MoveEntity_57FA0(&axis, yaw, pitch, 10240)` (EF:56059-64).
    // The pose is level (pitch 0), so the step is purely horizontal
    // and the aim point keeps the CASTER's own z — not the bolt's
    // fov-lifted muzzle z.
    let (ax, ay, az) = (
        lane(&w, slot, "dest_x").expect("dest_x is modelled"),
        lane(&w, slot, "dest_y").expect("dest_y is modelled"),
        lane(&w, slot, "dest_z").expect("dest_z is modelled"),
    );
    assert_ne!(
        (ax, ay, az),
        (0, 0, 0),
        "the port used to leave @0x9A at zero on every arm but the duel dart"
    );
    assert_eq!(
        az, pose.z as i64,
        "the origin is the CASTER's position (EF:56059), not the \
         fov-lifted muzzle, and a level pitch keeps its z"
    );
    let dx = wrap_d(ax as u16, pose.x) as f64;
    let dy = wrap_d(ay as u16, pose.y) as f64;
    let reach = (dx * dx + dy * dy).sqrt();
    assert!(
        (reach - 10240.0).abs() < 64.0,
        "EF:56064 steps the aim point 10240 units along the launch \
         bearing (LUT rounding aside); got {reach:.1}"
    );
    // The bolt itself is NOT at its aim point — the lane is a stored
    // destination, not the spawn.
    let bz = lane(&w, slot, "z").expect("z is modelled");
    assert!(
        bz > az,
        "the bolt is born at the fov-lifted muzzle, above the aim point's z"
    );
}

/// ROUND 104 — **THE TERRAIN-TAIL EIGHT.** The round-98 table
/// deliberately excluded the eight class-15 fire handlers whose
/// `axis_0x9A_154x` step ends in a ground read, on the grounds that
/// "a terrain lookup this helper cannot express". It can: it is a
/// third flag. All eight are the same three statements —
///
/// ```text
///   v->axis_0x9A_154x = caster->position_0x4C_76;
///   MoveEntity_57FA0(&v->axis_0x9A_154x, wizext@0x18 + caster.yaw,
///                    0, 4096);
///   v->axis_0x9A_154x.z = getTerrainAlt_10C40(&v->axis_0x9A_154x);
/// ```
///
/// — `sub_6B870` (15) EF:57382-83 · `sub_6BAB0` (16) EF:57455-56 ·
/// `sub_6BCF0` (17) EF:57527-28 · `sub_6BF30` (18) EF:57597-98 ·
/// `sub_6C3E0` (**20**) EF:57750-51 · `sub_6C620` (21) EF:57827-28 ·
/// `sub_6CAC0` (23, the mine) EF:57996-97 · `sub_6CFA0` (25)
/// EF:58161-62.
///
/// BYTE-VERIFIED in the shipped `NETHERW.EXE` (`off = 0x34800 +
/// linear − 0x10000`): `sub_6C3E0` at file **0x90BE0** carries
/// `0x90D44 lea edi,[edi+0x9a]` / `0x90D4A lea esi,[ebx+0x4c]` /
/// `movsl ; movsw` (the six-byte axis = caster.position copy), then
/// `0x90D56 push $0x1000` (**4096**) with `0x90D63 push $0x0` (pitch
/// **0**) and yaw `[ebx+0x1c] + [edx+0x18]` into `0x90D77 call
/// 0x7c7a0` (= `MoveEntity_57FA0`, linear 0x57FA0), then
/// `0x90D80 call 0x35440` (= `getTerrainAlt_10C40`, linear 0x10C40)
/// and `0x90D8B mov %ax,0x9e(%edx)` — @0x9E is the axis vector's z.
/// The same `push $0x1000` + `mov %ax,0x9e(...)` pair sits in all
/// seven siblings (0x90070 / 0x902B0 / 0x904F0 / 0x90730 / 0x90E20 /
/// 0x912C0 / 0x917A0).
///
/// WITNESS: mc2l22 t=63319 (the take's LAST head) — the human casts
/// GRAVITY WELL from (19801, 50761, 4724) at yaw 75 and retail
/// records the `(9,22)` at slot 816 with `dest (20735, 46773, 2324)`:
/// horizontal leg exactly 4096 at pitch 0, and 2324 is the ground
/// height there, not any polar z. The port left all three at zero.
/// mc2l0-spells-galore `MGC_RAW_SHADOW=1`: **325 rows fixed, 0
/// introduced**, every one a `dest_x`/`dest_y`/`dest_z` on a
/// `(9,2)`/`(9,4)`/`(9,5)`/`(9,22)`/`(9,23)`/`(9,26)`/`(9,29)`.
///
/// The lane is outside `EntObsMc2`, so this is a unit pin, not a
/// fixture. Kill switch: `MGC_NO_MC2_LAUNCH_AXIS_GROUND=1`.
#[test]
fn the_terrain_tail_arms_aim_4096_at_pitch_zero_and_snap_the_z_to_the_ground() {
    let Some(root) = baked_root() else {
        eprintln!("skipping: no baked data");
        return;
    };
    let Some(mut w) = build_world(&root) else {
        eprintln!("skipping: level-000 has no terrain");
        return;
    };
    w.set_dev_spells(true);
    let (cx, cy) = open_spot(&w);
    let pose = pose_at(&w, cx, cy);

    // Spell 20 = GRAVITY WELL (`sub_6C3E0`), the `(9,22)` carrier —
    // the witness's own arm.
    w.mc2_select_spell(20, 0, 0);
    let mut slot = None;
    for _ in 0..8 {
        w.tick(
            pose,
            PlayerCommand {
                fire_left: true,
                ..Default::default()
            },
        );
        slot = w
            .sprite_lane()
            .into_iter()
            .find(|(_, _, c, m)| *c == 9 && *m == 22)
            .map(|t| t.0);
        if slot.is_some() {
            break;
        }
    }
    let slot = slot.expect("the gravity-well carrier (9,22) was launched");

    let (ax, ay, az) = (
        lane(&w, slot, "dest_x").expect("dest_x is modelled"),
        lane(&w, slot, "dest_y").expect("dest_y is modelled"),
        lane(&w, slot, "dest_z").expect("dest_z is modelled"),
    );
    assert_ne!(
        (ax, ay, az),
        (0, 0, 0),
        "the round-98 table left every terrain-tail arm at the ctor's zero"
    );
    // `MoveEntity_57FA0(&axis, yaw, 0, 4096)` — pitch is a literal
    // zero on all eight sites (`push $0x0` at 0x90D63), so the step
    // is purely horizontal whatever the caster's pitch.
    let dx = wrap_d(ax as u16, pose.x) as f64;
    let dy = wrap_d(ay as u16, pose.y) as f64;
    let reach = (dx * dx + dy * dy).sqrt();
    assert!(
        (reach - 4096.0).abs() < 8.0,
        "EF:57751 steps the aim point 4096 units at PITCH 0; got {reach:.1}"
    );
    // `axis.z = getTerrainAlt_10C40(&axis)` — the ground under the
    // AIM POINT, not under the caster and not the polar z.
    assert_eq!(
        az,
        w.ground_z_engine(ax as u16, ay as u16) as i64,
        "the tail is `axis.z = getTerrainAlt(&axis)` (EF:57751, \
         NETHERW.EXE 0x90D80 call 0x35440 / 0x90D8B mov %ax,0x9e(%edx))"
    );
    assert_ne!(
        az, pose.z as i64,
        "the caster is airborne, so a ground-snapped z must differ \
         from the polar step's (which would have kept pose.z)"
    );
}
