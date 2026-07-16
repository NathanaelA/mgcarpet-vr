//! The game shell: window, input, fixed-timestep loop.
//!
//! The carpet flyer. Loads a baked `.mgcl` package, resolves its color
//! LUT from the baked assets, and flies: the sim ticks at
//! `mgc_sim::TICK_RATE_HZ`, rendering interpolates between the last two
//! ticks at whatever rate the display runs.
//!
//! Also runs headless: `--screenshot out.png` renders one frame
//! offscreen and exits, which is how terrain changes get verified
//! without a display.

mod bakecheck;
mod campaign;
mod config;
mod entities;
mod settings;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mgc_formats::bundle::Bundle;
use mgc_formats::{Game, LevelPackage, mgcl};
use mgc_render::{Billboard, CameraView, LevelView, Renderer};
use mgc_sim::{FlightInput, Flyer, Simulation, TICK_DT};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const FOV_Y: f32 = 60.0_f32.to_radians();
const MOUSE_SENSITIVITY: f32 = 0.0022;
/// MC1 virtual-stick gain: stick units (±127 full deflection) per
/// pixel of mouse motion. The original's DOS cursor reached full
/// deflection ~160 px from the center of a 320-wide screen (~0.8/px);
/// half that suits modern DPI while sensitivity 1.0 keeps the range.
const STICK_PER_PIXEL: f32 = 0.4;
/// The book's canonical spell order (`byte_99B88`, remc1 :5752) —
/// retail's scan order for the level-init quickselect pre-seed
/// (:49216-59); identical in HW (remc1hw :4381).
const SPELL_CANON: [u8; 24] = [
    0, 3, 2, 16, 1, 14, 4, 12, 6, 9, 7, 8, 15, 18, 17, 19, 13, 5, 11, 10, 20, 21, 22, 23,
];

/// Pristine inputs to rebuild the [`mgc_sim::mc1::world::World`] for a
/// LEVEL RESTART — the original's castle-less-death "lost + level
/// over" flow ends in exactly this (respawn at the start of a fresh
/// level).
struct WorldInit {
    /// The sim-side game profile (chassis + verb column selector).
    game: mgc_sim::ids::GameId,
    planes: mgc_sim::mc1::features::Planes,
    things: Vec<mgc_formats::Thing>,
    seed: u32,
    assets: mgc_sim::mc1::features::FeatureAssets,
    win_pct: u16,
    /// Rival wizard configs by player slot (wizards.json) + the
    /// level's active-slot count. MC1 column only.
    wizards: [Option<mgc_sim::mc1::rivals::RivalConfig>; 8],
    /// MC2 rival configs by color (wizards.json MC2 shape + the
    /// header's authored castle levels); `player_count` doubles as
    /// the NumberOfPlayers bound (header unk09) on MC2.
    mc2_wizards: [Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8],
    player_count: u16,
    /// MC2 stage checkpoints (`(index, stage, x, y)` rows) — the
    /// single-stage objective engine's board. Empty for MC1/HW.
    stages: Vec<(i8, i16, i16, i16)>,
    /// MC2 StageVars (`(index, stage, x, y, data)` per slot) — the
    /// triggered-spawn / hold-gate layer. Empty for MC1/HW.
    stage_vars: Vec<(i8, i8, u8, u8, u32)>,
    /// MC2 Night/Cave level: the runtime terrain repaint inverts
    /// relief shading (remc2 Terrain.cpp:2030-2033).
    night_shade: bool,
    doom_level: bool,
    /// Draw stand-in art for unported models (MC2 default until its
    /// roster closes; the ledger stays truthful either way).
    placeholders: bool,
    /// Remove spell jars the local player already owns (P-class
    /// unfaithful improvement; both games). Applied at every level
    /// load — the sim self-culls owned jars on their next tick.
    prune_owned_jars: bool,
    /// The chassis constant set: the game's pristine profile, or a
    /// deliberately deviating one (the limit-removing `--pool-slots`
    /// dev flag; G-class — a run under a bumped pool is not a
    /// faithful fixture).
    chassis: mgc_sim::chassis::ChassisParams,
}

impl WorldInit {
    fn build(&self) -> mgc_sim::mc1::world::World {
        let mut w = mgc_sim::mc1::world::World::new_full(
            self.planes.clone(),
            &self.things,
            self.seed,
            self.assets.clone(),
            self.chassis.clone(),
            self.game,
        );
        // Applies to BOTH games' jar systems (P-class improvement).
        w.set_prune_owned_jars(self.prune_owned_jars);
        if matches!(self.game, mgc_sim::ids::GameId::Mc2) {
            w.set_placeholders(self.placeholders);
            w.set_mc2_night_shade(self.night_shade);
            w.set_mc2_doom_level(self.doom_level);
            if !self.stages.is_empty() {
                w.set_mc2_stages(&self.stages);
            }
            if !self.stage_vars.is_empty() {
                w.set_mc2_stagevars(&self.stage_vars);
            }
            w.set_mc2_wizards(&self.mc2_wizards, self.player_count);
        } else {
            if self.win_pct > 0 {
                w.set_win_pct(self.win_pct);
            }
            w.set_wizards(&self.wizards, self.player_count);
        }
        w
    }
}

/// Resolve the package's wizards.json into per-slot rival configs
/// (MC1: the 8 x 216-byte level-record tail — aggression/accuracy/
/// tempo + the two 24-spell masks; the AI's book = pregrant &&
/// allowed, remc1 :49222).
fn rival_configs(
    wizards: Option<&mgc_formats::Wizards>,
) -> ([Option<mgc_sim::mc1::rivals::RivalConfig>; 8], u16) {
    let mut out: [Option<mgc_sim::mc1::rivals::RivalConfig>; 8] = Default::default();
    let Some(w) = wizards else { return (out, 1) };
    let count = w.player_count.unwrap_or(1).min(8);
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(acc), Some(tempo), Some(allowed_mask)) =
            (cfg.accuracy, cfg.tempo, cfg.allowed_spells.as_ref())
        else {
            continue; // MC2-shaped config: no MC1 rival data
        };
        let mut book = [false; 24];
        let mut allowed = [false; 24];
        for s in 0..24 {
            let a = allowed_mask.get(s).copied().unwrap_or(0) != 0;
            allowed[s] = a;
            book[s] = a && cfg.starting_spells.get(s).copied().unwrap_or(0) != 0;
        }
        out[slot] = Some(mgc_sim::mc1::rivals::RivalConfig {
            aggression: cfg.aggression.clamp(0, 255) as u8,
            accuracy: acc.clamp(0, 255) as u8,
            tempo: tempo.clamp(0, 255) as u8,
            castle_level: cfg.castle_level.unwrap_or(0),
            book,
            allowed,
        });
    }
    (out, count)
}

/// Resolve an MC2 package's wizards.json + level header into
/// per-color rival configs: personality (aggression/perception/
/// reflexes/Life), the three 26-spell masks, the authored starting-
/// castle level (header `players[color]`), and the NumberOfPlayers
/// bound (header `unk09` — colors 1..n-1 spawn as rivals).
fn mc2_rival_configs(
    wizards: Option<&mgc_formats::Wizards>,
    header: Option<&mgc_formats::LevelHeader>,
) -> ([Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8], u16) {
    let mut out: [Option<mgc_sim::mc2::rivals::Mc2RivalConfig>; 8] = Default::default();
    let (Some(w), Some(h)) = (wizards, header) else {
        return (out, 1);
    };
    let count = h.number_of_players.clamp(1, 8) as u16;
    for (slot, cfg) in w.wizards.iter().enumerate().take(8).skip(1) {
        let (Some(reflexes), Some(perception)) = (cfg.reflexes, cfg.perception) else {
            continue; // MC1-shaped config: no MC2 rival data
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
        out[slot] = Some(mgc_sim::mc2::rivals::Mc2RivalConfig {
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

struct LoadedLevel {
    view: LevelView,
    height: Vec<u8>,
    label: String,
    /// The sim-side game profile — picks the sprite table the pose
    /// snapshot resolves through (MC1 stats vs MC2 params).
    game: mgc_sim::ids::GameId,
    /// Bundle sprite data for the renderer (index, atlas pixels).
    sprites: Option<(mgc_formats::bundle::SpriteIndex, Vec<u8>)>,
    /// World entities resolved to billboards (initial population).
    billboards: Vec<Billboard>,
    /// Entity dots for the overhead map (the original's 1px markers).
    map_dots: Vec<mgc_render::MapDot>,
    /// The level's player start (class-3 m4 marker): position and
    /// facing for the flyer; None on levels without one (MC2, dev
    /// leftovers) falls back to the flyer default.
    start: Option<Flyer>,
    /// The living MC1/HW world (triggers, dispositions, runtime
    /// terrain events); moved into the Simulation by App::new. None =
    /// static terrain (MC2, or --no-terrain-features).
    world: Option<mgc_sim::mc1::world::World>,
    /// Rebuild inputs for the castle-less-death level restart.
    world_init: Option<WorldInit>,
    /// Bundle palette, kept for runtime map-dot rebuilds.
    palette_rgba: [[u8; 4]; 256],
    /// The MC2 map-marker environment (team-colour table + map-type
    /// colours) from the level header; Day for MC1/HW.
    mc2_env: entities::Mc2MapEnv,
    /// The per-game audio bundle directory (`assets/mc1-audio` /
    /// `mc2-audio`), when baked.
    audio_dir: Option<PathBuf>,
    /// The level-music pick: MC2 = ONE looping XMI by MapType
    /// (mc2-night/day/cave — docs/traces/mc2-music-law.md); MC1
    /// stays the INTERIM cgame1-3 level cycle until its
    /// song-command source data is traced.
    music_track: Option<String>,
    /// 0-based level number = the `CdTracks_DB080` speech row
    /// (docs/traces/mc2-voiceover-triggers.md §4).
    level_number: u32,
    /// HSPR UI sprites composited to RGBA (spellbook/HUD); None when
    /// the bundle has no UI members (MC2 until its UI track).
    ui: Option<ui::UiAssets>,
    /// Live trigger/portal volumes for the opt-in map overlay.
    map_areas: Vec<mgc_render::MapArea>,
    /// Castle/balloon icon patches for the map marker pass.
    map_icons: entities::MapIcons,
    /// Live icon stamps (own castle/balloons), refreshed per tick.
    map_stamps: Vec<mgc_render::MapStamp>,
    /// MC2 objective-guide targets (blinking marks + steer arrow),
    /// refreshed per tick from the current objective. Empty off-MC2.
    objective_marks: Vec<mgc_render::ObjectiveMark>,
    /// The plausible-spellbook grant set (spell ids), computed from the
    /// campaign jars before this level when the instrument is on; empty
    /// otherwise. Granted into the world after init. MC1 arm.
    plausible_spells: Vec<u8>,
    /// The MC2 plausible-spellbook grants: `(spell, banked_xp)` per
    /// learned spell (MC2's book is XP-driven). Empty off-MC2 or when
    /// the instrument is off. Installed via `mc2_grant_plausible`.
    plausible_book_mc2: Vec<(u8, i32)>,
}

/// Resolve the world's live volumes into map overlay circles: amber =
/// fly-into triggers, red = kill-watchers, cyan = collected-item
/// triggers, violet = portals, green = MC2 stage checkpoints (the
/// authored route, for troubleshooting — player request).
fn map_areas(world: &mgc_sim::mc1::world::World) -> Vec<mgc_render::MapArea> {
    use mgc_sim::mc1::world::VolumeKind;
    world
        .active_volumes()
        .into_iter()
        .map(|v| mgc_render::MapArea {
            x: v.x,
            z: v.z,
            radius: v.radius,
            color: match v.kind {
                VolumeKind::Proximity => [255, 196, 32],
                VolumeKind::KillWatch => [255, 64, 64],
                VolumeKind::WinTrigger => [64, 208, 255],
                VolumeKind::Portal => [208, 96, 255],
                VolumeKind::Objective => [96, 255, 96],
            },
        })
        .collect()
}

/// Resolve the package plus its asset bundle into what the renderer and
/// sim consume. `tileset` overrides MC1's world-set choice: by default
/// MC1 campaign levels use `mc1-temperate` and Hidden Worlds levels
/// `mc1-arctic` (the original's only selector is the Hidden Worlds mode
/// flag — see ROADMAP "Arctic tileset selection").
///
/// `terrain_features` applies the original's load-time entity-driven
/// terrain pass (craters, canyons, walls, building flattening/painting
/// — mgc_sim::mc1::features) to the pristine baked terrain, as the engine
/// does. Off = the raw generator output, for comparison renders.
fn load_level(
    level_path: &Path,
    tileset: Option<u8>,
    terrain_features: bool,
    plausible_spellbook: bool,
    prune_owned_jars: bool,
    pool_slots: Option<usize>,
    awake_range: Option<u32>,
) -> Result<LoadedLevel, String> {
    let file =
        std::fs::File::open(level_path).map_err(|e| format!("{}: {e}", level_path.display()))?;
    let package: LevelPackage =
        mgcl::read(file).map_err(|e| format!("{}: {e}", level_path.display()))?;
    let terrain = package.terrain.as_ref().ok_or_else(|| {
        format!(
            "{}: package has no terrain (bake with the mc2-genlevel oracle available)",
            level_path.display()
        )
    })?;

    // Bundles live in the baked tree next to the per-game level dirs:
    // <baked>/<game>/level-NNN.mgcl, <baked>/assets/<variant>/. MC1's
    // selector is the Hidden Worlds mode flag (temperate/arctic); MC2's
    // is the level's environment (day/night/cave from level.json).
    let baked_root = level_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));
    let set = tileset.unwrap_or(match package.meta.game {
        Game::HiddenWorlds => 1,
        _ => 0,
    });
    let mut variant = if package.meta.game == Game::MagicCarpet2 {
        // Night splits on the header's gfx_type bit 1 into plain and
        // "fog" graphics (remc2 Level.cpp:890: PALF/BL32F variants).
        match package.header.as_ref().map(|h| (h.map_type, h.gfx_type)) {
            Some((mgc_formats::MapType::Night, g)) if g & 2 != 0 => "mc2-night-fog",
            Some((mgc_formats::MapType::Night, _)) => "mc2-night",
            Some((mgc_formats::MapType::Cave, _)) => "mc2-cave",
            _ => "mc2-day",
        }
    } else if set == 1 {
        "mc1-arctic"
    } else {
        "mc1-temperate"
    };
    // The MC2 map-marker environment (team-colour table + map-type
    // colours) follows the level header, independent of any bundle
    // fallback below.
    let mc2_env = if package.meta.game == Game::MagicCarpet2 {
        match package.header.as_ref().map(|h| h.map_type) {
            Some(mgc_formats::MapType::Night) => entities::Mc2MapEnv::Night,
            Some(mgc_formats::MapType::Cave) => entities::Mc2MapEnv::Cave,
            _ => entities::Mc2MapEnv::Day,
        }
    } else {
        entities::Mc2MapEnv::Day
    };
    if !baked_root.join("assets").join(variant).is_dir() && variant.starts_with("mc2") {
        eprintln!("note: {variant} bundle not baked — using mc1-temperate as a stand-in (rebake)");
        variant = "mc1-temperate";
    }
    let bundle = Bundle::load(&baked_root.join("assets").join(variant))
        .map_err(|e| format!("bundle {variant}: {e}"))?;

    let mut palette = [[0u8; 3]; 256];
    for (i, rgb) in palette.iter_mut().enumerate() {
        rgb.copy_from_slice(&bundle.palette[i][..3]);
    }

    let game = match package.meta.game {
        Game::MagicCarpet1 => "mc1",
        Game::HiddenWorlds => "mc1hw",
        Game::MagicCarpet2 => "mc2",
    };

    let mut height = terrain.height.clone();
    let mut tile_type = terrain.tile_type.clone();
    let mut shading = terrain.shading.clone();
    let mut angle = terrain.angle.clone();
    // MC2 cave second heightmap (empty off-cave / on pre-8 bakes).
    let ceiling = terrain.ceiling.clone().unwrap_or_default();

    // The living world: the load-time feature pass (MC1/HW — MC2
    // terrain is pre-generated, remc2 has no feature event loop),
    // then the init spawns — MC1's disposition-0 sweep / MC2's
    // GenerateEvents passes. Things authored behind triggers
    // (dis_id != 0 / DisId >= 0) stay latent until fired. Needs the
    // shading + angle planes and feature-pass data.
    let game_id = mgc_sim::ids::GameId::from(package.meta.game);
    let is_mc2 = matches!(game_id, mgc_sim::ids::GameId::Mc2);
    let mut world = None;
    let mut world_init = None;
    if terrain_features {
        // Feature-pass assets: every game reads them from its own
        // bundle (mc2 bundles carry SEARCH + the BUILD0-0 footprint
        // bank since epoch 3, plus BLDGPRM for the building creator);
        // an old mc2 bake falls back to the mc1-temperate stand-in so
        // the world still lives.
        let mut feature_src = (
            bundle.search.clone(),
            bundle.build_tab.clone(),
            bundle.build_dat.clone(),
        );
        if is_mc2 && (feature_src.1.is_none() || feature_src.2.is_none()) {
            eprintln!("note: mc2 bundle lacks build data — mc1-temperate stand-in (rebake)");
            if let Ok(b) = Bundle::load(&baked_root.join("assets").join("mc1-temperate")) {
                feature_src = (b.search, b.build_tab, b.build_dat);
            }
        }
        match (&shading, &angle, feature_src) {
            (Some(sh), Some(an), (Some(search), Some(build_tab), Some(build_dat))) => {
                let mut assets =
                    mgc_sim::mc1::features::FeatureAssets::parse(&search, &build_tab, &build_dat)?;
                if let Some(prm) = bundle.bldgprm.as_deref() {
                    assets = assets.with_bldgprm(prm);
                }
                if let Some(sp) = bundle.spells.as_deref() {
                    assets = assets.with_spells(sp)?;
                }
                // The retail load-time sprite-extents derivation
                // (remc2 EF:44870-44910): collision boxes come from
                // the sprite bitmaps' aspect — the static param
                // table alone leaves most speed_6 at 0 (zero-box).
                if is_mc2 && let Some((sidx, _)) = bundle.sprites.as_ref() {
                    let dims: Vec<(u16, u16)> =
                        sidx.sprites.iter().map(|e| (e.width, e.height)).collect();
                    assets = assets.with_mc2_sprite_ext(mgc_sim::mc2::derive_sprite_extents(&dims));
                }
                let seed = package.gen_params.as_ref().map_or(0, |g| g.seed);
                // The MC1 level goal: footer[0] = the required banked
                // percentage of world mana (level offset 38800 —
                // the win check's threshold and the HUD goal tick).
                // MC2's win lives on the stage board instead.
                let win_pct = package
                    .gen_params
                    .as_ref()
                    .and_then(|g| g.footer)
                    .map_or(0, |f| f[0]);
                let (wizards, mc1_count) = rival_configs(package.wizards.as_ref());
                let (mc2_wizards, mc2_count) =
                    mc2_rival_configs(package.wizards.as_ref(), package.header.as_ref());
                let player_count = if is_mc2 { mc2_count } else { mc1_count };
                let stages = package
                    .stages
                    .as_ref()
                    .map(|st| {
                        st.checkpoints
                            .iter()
                            .map(|c| (c.index, c.stage, c.x, c.y))
                            .collect()
                    })
                    .unwrap_or_default();
                let stage_vars = package
                    .stages
                    .as_ref()
                    .map(|st| {
                        st.variables
                            .iter()
                            .map(|v| (v.index, v.stage, v.x, v.y, v.data))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut chassis = game_id.chassis();
                if let Some(n) = pool_slots {
                    chassis.pool_slots = n;
                    println!(
                        "chassis: pool_slots {n} (limit-removing override; \
                         G-class — not a faithful run)"
                    );
                }
                if let Some(tiles) = awake_range {
                    // 0 = always awake; otherwise (tiles·256)² with a
                    // saturate — ≥128 tiles exceeds the torus's max
                    // shortest-wrap distance, so it saturates to
                    // always-awake too.
                    chassis.awake_gate_sq = if tiles == 0 {
                        i32::MAX
                    } else {
                        ((tiles as i64 * 256).pow(2)).min(i32::MAX as i64) as i32
                    };
                    println!(
                        "chassis: awake_range {} (faithful = 24 tiles; \
                         G-class — not a faithful run)",
                        if tiles == 0 {
                            "off (always awake)".to_string()
                        } else {
                            format!("{tiles} tiles")
                        }
                    );
                }
                let init = WorldInit {
                    game: game_id,
                    planes: mgc_sim::mc1::features::Planes {
                        height: height.clone(),
                        tile_type: tile_type.clone(),
                        shading: sh.clone(),
                        angle: an.clone(),
                        ceiling: ceiling.clone(),
                    },
                    things: package.things.things.clone(),
                    seed,
                    assets,
                    win_pct,
                    wizards,
                    mc2_wizards,
                    player_count,
                    stages,
                    stage_vars,
                    placeholders: is_mc2,
                    prune_owned_jars,
                    night_shade: is_mc2
                        && matches!(
                            package.header.as_ref().map(|h| h.map_type),
                            Some(mgc_formats::MapType::Night) | Some(mgc_formats::MapType::Cave)
                        ),
                    // The doom-palette bit (gfx_type & 2, the
                    // night-fog variant) gates the (5,10) doomsday
                    // pyramid's ctor (remc2 EF:33968).
                    doom_level: is_mc2
                        && package.header.as_ref().is_some_and(|h| h.gfx_type & 2 != 0),
                    chassis,
                };
                let w = init.build();
                // Truthful seam telemetry at boot: what still serves
                // through the MC1 fallback, and what spawned as a
                // stand-in (empty on MC1/HW by construction).
                let fallbacks = w.verb_fallbacks();
                if !fallbacks.is_empty() {
                    println!("verb fallbacks (MC1 arm serving): {}", fallbacks.join(", "));
                }
                for &(class, model, n) in w.misfits() {
                    println!("misfit: ({class},{model}) x{n} — unported model (placeholder art)");
                }
                // The view starts from the post-feature planes.
                height.copy_from_slice(&w.planes().height);
                tile_type.copy_from_slice(&w.planes().tile_type);
                shading
                    .as_mut()
                    .unwrap()
                    .copy_from_slice(&w.planes().shading);
                angle.as_mut().unwrap().copy_from_slice(&w.planes().angle);
                world = Some(w);
                world_init = Some(init);
            }
            (None, ..) | (_, None, _) => eprintln!(
                "note: package lacks shading/angle planes — terrain features skipped (rebake)"
            ),
            _ => eprintln!(
                "note: feature-pass data missing (bundle search/build) — living world skipped (rebake)"
            ),
        }
    }

    // World entities as billboards + map dots. With a live world, the
    // sim's pose snapshot is the source of truth (sprite types, spawn
    // facing and jitter come from the ported spawn handlers), resolved
    // through the game's own sprite table; without one
    // (--no-terrain-features), every drawable record resolves
    // statically — the old behavior, kept as the comparison mode
    // (MC1/HW only; MC2 has no static resolver).
    let (billboards, map_dots) = {
        let index = bundle.sprites.as_ref().map(|(i, _)| i);
        let dims = |id: u16| {
            index
                .and_then(|i| i.sprites.get(id as usize))
                .map(|s| (s.width, s.height, s.flags))
        };
        match &world {
            Some(w) => {
                let poses = w.live_poses();
                (
                    entities::billboards_from_poses(game_id, &poses, dims),
                    // No dwelling is claimed at load time, so the
                    // owned-buildings highlight is vacuously off here
                    // (and the blink phase starts low).
                    entities::map_dots_from_poses(
                        game_id,
                        &poses,
                        &bundle.palette,
                        false,
                        mc2_env,
                        0,
                    ),
                )
            }
            None if !is_mc2 => (
                entities::billboards(&package.things.things, &height, dims),
                entities::map_dots(&package.things.things, &bundle.palette),
            ),
            None => (Vec::new(), Vec::new()),
        }
    };
    if is_mc2 {
        // Boot telemetry while the MC2 roster is open: how much of
        // the live population resolved to drawables.
        println!(
            "mc2 boot: {} billboards / {} live poses",
            billboards.len(),
            world.as_ref().map_or(0, |w| w.live_poses().len())
        );
    }

    // The original's spawn: the start marker's position (MC1 class-3
    // m4 / MC2 (10,0x52)), hovering over the (post-feature) terrain,
    // facing north.
    let start = entities::player_start(game_id, &package.things.things).map(|(x, z)| Flyer {
        x,
        y: entities::ground_at(&height, x, z) + entities::START_HOVER,
        z,
        yaw: 0.0,
        pitch: 0.0,
        ..Flyer::default()
    });

    let ui_assets = bundle.ui_sprites.as_ref().map(|(idx, px)| {
        ui::UiAssets::build(
            idx.clone(),
            px,
            &bundle.palette,
            bundle.blend_lut.as_deref(),
            // MC1 pre-composites its book tiles; MC2's sprite ids map
            // to the selector pane instead (drawn directly).
            !is_mc2,
            bundle.font.as_ref().map(|(i, p)| (i, p.as_slice())),
        )
    });

    // Per-game audio bundle + the music pick. MC2: ONE looping XMI
    // by MapType (Night=GAME1, Day=GAME2, Cave=GAME3 — EF:31441-49,
    // docs/traces/mc2-music-law.md); the redbook tracks are speech,
    // never gameplay music. MC1 stays the INTERIM level cycle until
    // its song-command source data is traced.
    let audio_game = if package.meta.game == Game::MagicCarpet2 {
        "mc2"
    } else {
        "mc1"
    };
    let audio_dir = {
        let d = baked_root
            .join("assets")
            .join(format!("{audio_game}-audio"));
        d.is_dir().then_some(d)
    };
    let music_track = Some(if audio_game == "mc2" {
        match package.header.as_ref().map(|h| h.map_type) {
            Some(mgc_formats::MapType::Night) => "mc2-night".to_string(),
            Some(mgc_formats::MapType::Cave) => "mc2-cave".to_string(),
            _ => "mc2-day".to_string(),
        }
    } else {
        format!("cgame{}", 1 + package.meta.level as usize % 3)
    });

    // Plausible spellbook (playtest instrument): the union of spell
    // jars in the campaign levels before this one. Only scanned when
    // the toggle is on — it reads the sibling `level-NNN.mgcl` files.
    // MC2 arm: the XP-driven book. Reads the same sibling files, but
    // unions class-15 jars → learned set and counts class-14 scrolls →
    // banked XP (see campaign::plausible_spellbook_mc2). Archive-index
    // order — MC2 has no campaign-progression data (logged honestly).
    let plausible_book_mc2 = if plausible_spellbook && package.meta.game == Game::MagicCarpet2 {
        let dir = level_path.parent().unwrap_or(Path::new("."));
        let p = campaign::plausible_spellbook_mc2(dir, &package);
        println!(
            "plausible-spellbook (MC2): {} spell(s) at ~{} XP each from {} scroll(s) across {} \
             level(s) before level {} (archive-index order — MC2 has no verified campaign \
             route){}",
            p.grants.len(),
            p.grants.first().map_or(0, |g| g.1),
            p.scroll_count,
            p.scanned_levels.len(),
            package.meta.level,
            if p.skipped_levels.is_empty() {
                String::new()
            } else {
                format!(" (skipped unreadable levels: {:?})", p.skipped_levels)
            },
        );
        p.grants
    } else {
        Vec::new()
    };

    let plausible_spells = if plausible_spellbook && package.meta.game == Game::MagicCarpet1 {
        let dir = level_path.parent().unwrap_or(Path::new("."));
        let p = campaign::plausible_spellbook(dir, &package);
        let names: Vec<&str> = p
            .spells
            .iter()
            .map(|&s| mgc_sim::mc1::spells::SpellId(s).name())
            .collect();
        println!(
            "plausible-spellbook: {} spell(s) from {} campaign level(s) before level {} \
             [{}]{}{}",
            p.spells.len(),
            p.scanned_levels.len(),
            package.meta.level,
            names.join(", "),
            if p.skipped_levels.is_empty() {
                String::new()
            } else {
                format!(" (skipped unreadable levels: {:?})", p.skipped_levels)
            },
            if p.masked.is_empty() {
                String::new()
            } else {
                // The level's availability mask (retail :49229) strips
                // these at level start — rediscover them in play.
                format!(
                    " (level mask strips: {})",
                    p.masked
                        .iter()
                        .map(|&s| mgc_sim::mc1::spells::SpellId(s).name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        );
        p.spells
    } else {
        Vec::new()
    };

    Ok(LoadedLevel {
        view: LevelView {
            tile_type,
            height: height.clone(),
            shading,
            palette,
            tile_colors: bundle.tile_colors,
            shade_lut: bundle.shade_lut,
            atlas: bundle.terrain_atlas.map(|(_, data)| data),
            angle,
            wave: match package.meta.game {
                Game::MagicCarpet2 => mgc_render::WaveMode::Mc2,
                _ => mgc_render::WaveMode::Mc1,
            },
            ceiling: (!ceiling.is_empty()).then(|| ceiling.clone()),
        },
        height,
        label: format!("{game} level {}", package.meta.level),
        game: game_id,
        sprites: bundle.sprites,
        billboards,
        map_dots,
        start,
        map_areas: world.as_ref().map(map_areas).unwrap_or_default(),
        world,
        world_init,
        palette_rgba: bundle.palette,
        mc2_env,
        map_icons: entities::MapIcons {
            // Castle = UI sprite 58+team, balloon = 66+team, all
            // eight teams; remc1 sub_48710 :57230/:57234.
            castle: std::array::from_fn(|t| ui_assets.as_ref().and_then(|u| u.map_stamp(58 + t))),
            balloon: std::array::from_fn(|t| ui_assets.as_ref().and_then(|u| u.map_stamp(66 + t))),
            // Spell icons shrunk to marker size, floating over the
            // jar dot — the expose-jar-spells debug stamps (drawn
            // only when that option is on).
            spell: (0..26u8)
                .map(|s| {
                    let id = ui::spell_icon_sprite(game_id, s)?;
                    let mut st = ui_assets.as_ref().and_then(|u| u.map_stamp(id))?;
                    let f = 12.0 / st.w.max(st.h) as f32;
                    if f < 1.0 {
                        st.w = ((st.w as f32 * f) as u32).max(1);
                        st.h = ((st.h as f32 * f) as u32).max(1);
                    }
                    st.anchor = [0.5, 1.0];
                    Some(st)
                })
                .collect(),
        },
        map_stamps: Vec::new(),
        objective_marks: Vec::new(),
        plausible_book_mc2,
        ui: ui_assets,
        audio_dir,
        music_track,
        level_number: package.meta.level,
        plausible_spells,
    })
}

/// Currently-held key axes, sampled into a `FlightInput` per tick.
#[derive(Default)]
struct HeldKeys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    turn_left: bool,
    turn_right: bool,
    pitch_up: bool,
    pitch_down: bool,
}

/// Mouse deltas accumulated since the last tick.
#[derive(Default)]
struct MouseAccum {
    yaw: f32,
    pitch: f32,
}

/// The MC1 model's virtual stick: mouse motion integrates into a
/// POSITION offset from center (the original reads the DOS cursor's
/// screen offset, ±127 per axis — an airplane-stick input: deflection
/// = turn rate, re-center to fly straight). Kept in floats app-side;
/// sampled to the sim's i16 pair each tick.
#[derive(Default)]
struct VirtualStick {
    x: f32,
    y: f32,
}

struct App {
    level: LoadedLevel,
    /// The single resolved options source of truth (defaults + config
    /// file + CLI overrides, merged in `main`). Every option is read
    /// live off this struct; runtime keys mutate it and re-apply, so a
    /// future in-game menu drives the exact same path. See the
    /// `settings` registry for the option taxonomy.
    cfg: config::Config,
    /// Pickable-jar positions `(x, alt, z, spell)` for the floating
    /// main-view icons; rebuilt with the pose snapshot, empty when
    /// `render.enhancement.expose_jar_spells` is off.
    jar_markers: Vec<(f32, f32, f32, u8)>,
    /// Ticks since the mouse last moved — the retail MC2 "fly
    /// assistant" (PlayerInput.cpp:2001-09): 0x30 idle polls with no
    /// action pending recenter the cursor, i.e. our virtual stick.
    /// Without it the grabbed stick rests wherever the last flick
    /// left it — a permanent invisible deflection (the player's
    /// "level flight declines to the very ground": a parked stick_y
    /// of 5+ units defeats the sine-LUT truncation that makes true
    /// near-level flight hold altitude). Faithful for MC2;
    /// enhancement-class in MC1/HW like Backspace.
    stick_idle_ticks: u16,
    /// Space pressed since the last sim tick (respawn confirm).
    pending_full_stop: bool,
    pending_respawn: bool,
    /// Shift+L pressed since the last sim tick (castle demolish).
    pending_demolish: bool,
    /// Which spell-selection surfaces are live (config
    /// `spell_selector` resolved against the running game): the MC1
    /// map-screen spellbook and/or the MC2 CTRL-hold pane.
    selector: config::SelectorSurfaces,
    /// CTRL currently held (the MC2 selector pane is hold-to-show,
    /// release-to-close — remc2 PI:505/PI:895).
    ctrl_held: bool,
    /// Whether the cursor was grabbed when CTRL went down, so release
    /// restores THAT state instead of force-grabbing (the cursor may
    /// have been deliberately freed via Escape or focus loss).
    ctrl_grab_restore: bool,
    /// The pane's per-game shape; None when `selector.ctrl_pane` is
    /// off.
    pane: Option<ui::SelectorPane>,
    /// Pane hit under the cursor, refreshed per frame while it's up.
    selector_hover: ui::SelectorHover,
    /// A held pane click: (grid slot, hand 0=L/1=R). The flyout
    /// live-tracks the hovered level until release commits it.
    selector_drag: Option<(usize, u8)>,
    /// Pane spell id last bound to each hand (the pane's corner
    /// tags; MC2 only — MC1 reads the loadout directly).
    pane_bound: [Option<u8>; 2],
    /// Per-spell SELECTED LEVEL (MC2 mechanic, `array_0x437` in the
    /// original: one persistent level per spell, reused by every
    /// selection route). Indexed by pane spell id; MC1 spells are
    /// single-level so it stays all-zero there. App-side until the
    /// MC2 spell column lands sim-side (Phase 4.2).
    spell_levels: [u8; 26],
    /// Sim tick of the last map-texture recompose (dots/blink are
    /// tick-derived, so update_map runs per tick, not per frame).
    last_map_tick: Option<u64>,
    /// P-key pause: the sim clock freezes, rendering and UI stay live.
    paused: bool,
    /// Own castle position in tile units (the guide-path target),
    /// refreshed from the pose set.
    castle_pos: Option<(f32, f32)>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    sim: Simulation,
    prev_flyer: Flyer,
    keys: HeldKeys,
    mouse: MouseAccum,
    stick: VirtualStick,
    /// Left/right button held while grabbed: the two casting hands.
    fire_held: bool,
    fire_right_held: bool,
    grabbed: bool,
    /// Cursor position in window pixels (book-screen interactions).
    cursor: (f32, f32),
    /// Spell under the cursor on the book screen (display hit test,
    /// refreshed each frame the book is open).
    hovered: Option<mgc_sim::mc1::spells::SpellId>,
    /// Quick-key bindings 1..9,0 → spell id (session-local; set in the
    /// book by hovering + pressing a digit, or auto-assigned on spell
    /// acquisition like retail, :64858-67). Manual rebinding beyond
    /// the book's Ctrl+]+digit chord is our enhancement.
    quick_binds: [Option<u8>; 10],
    /// Last tick's owned-spell set — the acquisition edge detector
    /// feeding the retail quickselect auto-assign (app-side only,
    /// never part of the sim hash).
    prev_owned: [bool; 24],
    /// Equip requests to feed the next sim tick (LMB hand, RMB hand).
    pending_equip: (Option<u8>, Option<u8>),
    /// Pending MC2 pane commit: (spell, tier, hand) — the sim's
    /// PlayerAction 0x1F/0x20 equivalent (PlayerCommand.mc2_select).
    pending_mc2_select: Option<(u8, u8, u8)>,
    shift_held: bool,
    last_frame: std::time::Instant,
    accumulator: f32,
    /// Running pool-exhaustion drop count for this level (the
    /// limit-removing telemetry's playthrough readout).
    pool_dropped_total: u32,
    /// Misfit-ledger entries already reported (the spawn seam's
    /// graceful-degradation telemetry — unknown (class, model)
    /// things; mgc-sim ROADMAP "MULTI-GAME ARCHITECTURE" Phase 2).
    misfits_reported: usize,
    /// Audio runtime (None in headless paths / when opening failed).
    audio: Option<mgc_audio::Audio>,
    /// The end-of-game fadeout, armed when the sim reports the level
    /// WON (`World::won`): alpha 0→1 over ~0.8 s, then the app exits
    /// — the player-directed ending (2026-07-16): no stats screen,
    /// no menu return, campaign stitching comes later. MC2's ending
    /// already fades sim-side (`World::end_fade`); this rides on top
    /// so both games leave through the same door.
    quit_fade: Option<f32>,
}

impl App {
    fn new(mut level: LoadedLevel, cfg: config::Config) -> Self {
        let is_mc2 = matches!(level.game, mgc_sim::ids::GameId::Mc2);
        // Audio: open the device, load the game's audio bundle, start
        // the level music. Any failure degrades to silence, never to
        // an unplayable game.
        let mut audio = None;
        if cfg.audio.sound || cfg.audio.music {
            let mut a = mgc_audio::Audio::open();
            a.set_prefer_gm(cfg.audio.arrangement.prefer_gm());
            if is_mc2 {
                a.set_mc2_danger_ramp();
            }
            if let Some(dir) = &level.audio_dir {
                if let Err(e) = a.load_bundle(dir, 0) {
                    eprintln!("note: audio bundle: {e}");
                }
            } else {
                eprintln!("note: no audio bundle baked — sound effects disabled (rebake)");
            }
            a.set_volumes(
                if cfg.audio.sound {
                    cfg.audio.sfx_volume
                } else {
                    0.0
                },
                if cfg.audio.music {
                    cfg.audio.music_volume
                } else {
                    0.0
                },
            );
            if cfg.audio.music {
                if let Some(track) = &level.music_track {
                    if let Err(e) = a.play_music(track, true) {
                        eprintln!("note: music: {e}");
                    }
                }
            }
            audio = Some(a);
        }
        let mut sim = match level.world.take() {
            Some(w) => Simulation::with_world(w),
            None => Simulation::with_terrain(level.height.clone()),
        };
        sim.thrust_model = match cfg.controls.models.thrust {
            config::ThrustModel::Mc1 => mgc_sim::ThrustModel::Mc1,
            config::ThrustModel::Enhanced => mgc_sim::ThrustModel::Enhanced,
        };
        sim.altitude_model = match cfg.controls.models.altitude {
            config::AltitudeModel::Faithful => mgc_sim::AltitudeModel::Faithful,
            config::AltitudeModel::ExtendedLift => mgc_sim::AltitudeModel::ExtendedLift,
        };
        if let Some(start) = level.start {
            sim.flyer = start;
            sim.sync_carpet_from_flyer();
        }
        if let Some(w) = &mut sim.world {
            apply_instruments(
                w,
                cfg.gameplay.cheat.dev_spells,
                &level.plausible_spells,
                &level.plausible_book_mc2,
                cfg.gameplay.cheat.invincible,
            );
        }
        // Which spell-selection surfaces are live, resolved against the
        // running game (MC2 owns exactly the CTRL pane).
        let selector = cfg.gameplay.enhancement.spell_selector.resolve(is_mc2);
        let pane = selector.ctrl_pane.then(|| {
            if is_mc2 {
                ui::SelectorPane::mc2()
            } else {
                ui::SelectorPane::mc1()
            }
        });
        let prev_flyer = sim.flyer;
        Self {
            level,
            cfg,
            jar_markers: Vec::new(),
            stick_idle_ticks: 0,
            pending_full_stop: false,
            pending_respawn: false,
            pending_demolish: false,
            selector,
            ctrl_held: false,
            ctrl_grab_restore: false,
            pane,
            selector_hover: ui::SelectorHover::default(),
            selector_drag: None,
            pane_bound: [None; 2],
            spell_levels: [0; 26],
            last_map_tick: None,
            paused: false,
            castle_pos: None,
            window: None,
            renderer: None,
            sim,
            prev_flyer,
            keys: HeldKeys::default(),
            mouse: MouseAccum::default(),
            stick: VirtualStick::default(),
            fire_held: false,
            fire_right_held: false,
            grabbed: false,
            cursor: (0.0, 0.0),
            hovered: None,
            quick_binds: [None; 10],
            prev_owned: [false; 24],
            pending_equip: (None, None),
            pending_mc2_select: None,
            shift_held: false,
            last_frame: std::time::Instant::now(),
            accumulator: 0.0,
            pool_dropped_total: 0,
            misfits_reported: 0,
            audio,
            quit_fade: None,
        }
    }

    /// The HUD blends over the sky (faithful MC1) vs opaque solid
    /// panels (the MC2 readability toggle). Derived live from
    /// `render.enhancement.hud_transparency`.
    fn hud_transparent(&self) -> bool {
        matches!(
            self.cfg.render.enhancement.hud_transparency,
            config::HudTransparency::Mc1
        )
    }

    /// Per-sim-tick audio: drain the world's sound requests into the
    /// faithful mixer, feed the ambient rule, run the flush.
    fn audio_tick(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        let f = &self.sim.flyer;
        let pose = mgc_sim::mc1::world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, 0.0);
        let listener = mgc_audio::Listener {
            pos: (pose.x, pose.y, pose.z),
            yaw: pose.heading,
        };
        if let Some(w) = &mut self.sim.world {
            let frame = w.take_audio(pose);
            if self.cfg.audio.sound {
                for e in frame.events {
                    let source = if e.player {
                        mgc_audio::Source::Player
                    } else {
                        // e.tag = the emitter's OWNER word (resolved
                        // by take_audio) — the channel-pair key (D2).
                        mgc_audio::Source::World {
                            pos: e.pos,
                            owner: e.tag,
                        }
                    };
                    audio.event(e.id, source, &listener);
                }
                audio
                    .mixer
                    .set_ambient(frame.over_water, frame.fire_near, frame.market_near);
            }
            audio.set_danger(frame.danger);
            // MC2 objective voiceover: the sim's trigger ramp hands
            // over the SEGMENT; the row is the level number. Special
            // levels 30-34 address row 0 (seg 4) / row 10 (seg 9) —
            // retail EF:41020-29, ported verbatim.
            if let Some(seg) = frame.speech {
                if self.cfg.audio.speech {
                    let lvl = self.level.level_number;
                    let (row, seg) = if (30..=34).contains(&lvl) {
                        if seg == 9 { (10, 9) } else { (0, 4) }
                    } else {
                        (lvl, u32::from(seg))
                    };
                    if let Err(e) = audio.play_speech(row, seg) {
                        eprintln!("note: speech: {e}");
                    }
                }
            }
        }
        audio.tick();
    }

    /// The screen-mode chime (sub_3DC90 :49072, sound 14 at the
    /// local wizard): level start, map/book enter + exit, respawn.
    /// The sim-side switches emit it through the event stream; this
    /// is the app-side path for view toggles the sim never sees.
    fn ui_ding(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        if !self.cfg.audio.sound {
            return;
        }
        let f = &self.sim.flyer;
        let pose = mgc_sim::mc1::world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, 0.0);
        let listener = mgc_audio::Listener {
            pos: (pose.x, pose.y, pose.z),
            yaw: pose.heading,
        };
        audio.event(14, mgc_audio::Source::Player, &listener);
    }

    /// Castle-less death: rebuild the pristine world (the original
    /// restarts the level) and reset the flyer to the level start.
    fn restart_level(&mut self) {
        let Some(init) = &self.level.world_init else {
            return;
        };
        let mut w = init.build();
        self.pool_dropped_total = 0;
        self.misfits_reported = 0;
        // Retail wipes + reseeds the quick keys at level init
        // (:49216-59) — the acquisition diff below re-seeds the
        // starting spells in canonical order on the first tick.
        self.quick_binds = [None; 10];
        self.prev_owned = [false; 24];
        apply_instruments(
            &mut w,
            self.cfg.gameplay.cheat.dev_spells,
            &self.level.plausible_spells,
            &self.level.plausible_book_mc2,
            self.cfg.gameplay.cheat.invincible,
        );
        w.terrain_dirty = true;
        w.entities_dirty = true;
        let (thrust, altitude) = (self.sim.thrust_model, self.sim.altitude_model);
        self.sim = Simulation::with_world(w);
        self.sim.thrust_model = thrust;
        self.sim.altitude_model = altitude;
        if let Some(start) = self.level.start {
            self.sim.flyer = start;
            self.sim.sync_carpet_from_flyer();
        }
        self.prev_flyer = self.sim.flyer;
        self.castle_pos = None;
        self.sync_world();
        println!("level restarted (died without a castle)");
    }

    fn book_open(&self) -> bool {
        self.renderer.as_ref().is_some_and(|r| r.map_view())
    }

    fn tick_input(&mut self) -> FlightInput {
        let axis = |neg: bool, pos: bool| (pos as i32 - neg as i32) as f32;
        let k = &self.keys;
        // Keyboard turn rate: radians per tick (enhanced model only).
        let key_turn = 2.2 * TICK_DT;
        let book = self.book_open();
        let mc1 = self.cfg.controls.models.thrust == config::ThrustModel::Mc1;
        // Explicit float up/down is the extended-lift enhancement; the
        // faithful altitude model has no vertical control at all.
        let lift_keys = self.cfg.controls.models.altitude == config::AltitudeModel::ExtendedLift;
        let mut input = FlightInput {
            thrust: axis(k.back, k.forward),
            strafe: axis(k.left, k.right),
            lift: if lift_keys { axis(k.down, k.up) } else { 0.0 },
            yaw_delta: axis(k.turn_left, k.turn_right) * key_turn + self.mouse.yaw,
            pitch_delta: axis(k.pitch_down, k.pitch_up) * key_turn + self.mouse.pitch,
            // The book screen swallows the fire buttons (they bind
            // spells there, as in the original's map-screen input).
            fire_left: self.fire_held && self.grabbed && !book,
            fire_right: self.fire_right_held && self.grabbed && !book,
            equip_left: self
                .pending_equip
                .0
                .take()
                .map(mgc_sim::mc1::spells::SpellId),
            equip_right: self
                .pending_equip
                .1
                .take()
                .map(mgc_sim::mc1::spells::SpellId),
            mc2_select: self.pending_mc2_select.take(),
            full_stop: std::mem::take(&mut self.pending_full_stop),
            respawn: std::mem::take(&mut self.pending_respawn),
            demolish: std::mem::take(&mut self.pending_demolish),
            ..Default::default()
        };
        if mc1 {
            // The retail MC2 fly assistant (PlayerInput.cpp:2001-09):
            // mouse untouched and no action pending for 0x30
            // consecutive polls recenters the cursor — our virtual
            // stick. Retail gates on the raw position + pending
            // action bytes; ours on the motion-reset counter + held
            // fire. Game-keyed default (player ruling 2026-07-16):
            // MC2 = retail option on, MC1/HW = authentically absent
            // (parked-cursor deflections persist, as retail MC1's
            // visible-cursor scheme did) — `fly_assistant: on` opts
            // the enhancement in everywhere.
            let assist = self
                .cfg
                .controls
                .preferences
                .fly_assistant
                .enabled(matches!(self.level.game, mgc_sim::ids::GameId::Mc2));
            if !assist || input.fire_left || input.fire_right {
                self.stick_idle_ticks = 0;
            } else if self.stick.x != 0.0 || self.stick.y != 0.0 {
                self.stick_idle_ticks = self.stick_idle_ticks.saturating_add(1);
                if self.stick_idle_ticks > 0x30 {
                    self.stick = VirtualStick::default();
                    self.stick_idle_ticks = 0;
                }
            }
            // The MC1 model steers from the virtual stick; the delta
            // accumulators stay zero (the sim ignores them, but keep
            // the recorded input honest for future replays).
            input.stick_x = self.stick.x.round() as i16;
            input.stick_y = self.stick.y.round() as i16;
            input.yaw_delta = 0.0;
            input.pitch_delta = 0.0;
        }
        if book {
            // The original's map/book modes write NO movement input
            // (:20635-:20744 never reach the mouse read or command 6)
            // — the steering filters decay to center while the speed
            // target persists (the "map fixes your orientation, not
            // your velocity" behavior).
            input.thrust = 0.0;
            input.strafe = 0.0;
            input.lift = 0.0;
            input.stick_x = 0;
            input.stick_y = 0;
            input.yaw_delta = 0.0;
            input.pitch_delta = 0.0;
        }
        self.mouse = MouseAccum::default();
        input
    }

    /// Push runtime world changes (dug terrain, moving/spawned/removed
    /// entities) to the renderer. Entities move every tick now, so the
    /// billboard set refreshes per tick from the sim's pose snapshot;
    /// the map texture recompose (dots baked into it) is throttled to
    /// every 8th tick unless terrain actually changed.
    fn sync_world(&mut self) {
        let Some(w) = &mut self.sim.world else { return };
        for slot in w.take_rival_deaths() {
            // The retail death broadcast ("%name% <str 54>",
            // :55499-517) — the sim raises the on-screen toast at the
            // moment of death (game-aware name table); this console
            // line is a dev-log echo, so pick the matching table too.
            let name = match self.level.game {
                mgc_sim::ids::GameId::Mc2 => mgc_sim::mc2::rivals::MC2_RIVAL_NAMES,
                _ => mgc_sim::mc1::rivals::RIVAL_NAMES,
            }
            .get(slot as usize)
            .copied()
            .unwrap_or("?");
            eprintln!("{name} is dead");
        }
        let terrain = w.terrain_dirty;
        let entities = w.entities_dirty;
        if terrain {
            let (Some(shading), Some(angle)) = (
                self.level.view.shading.as_mut(),
                self.level.view.angle.as_mut(),
            ) else {
                return;
            };
            w.copy_planes_into(mgc_sim::mc1::features::TerrainPlanes {
                height: &mut self.level.view.height,
                tile_type: &mut self.level.view.tile_type,
                shading,
                angle,
            });
            // The live cave ceiling (pillars, Cave-In, the eases).
            if let Some(c) = self.level.view.ceiling.as_mut() {
                let live = w.ceiling_plane();
                if live.len() == c.len() {
                    c.copy_from_slice(live);
                }
            }
        }
        let mut bars = Vec::new();
        if entities {
            let poses = w.live_poses();
            let index = self.level.sprites.as_ref().map(|(i, _)| i);
            let dims = |id: u16| {
                index
                    .and_then(|i| i.sprites.get(id as usize))
                    .map(|s| (s.width, s.height, s.flags))
            };
            self.level.billboards = entities::billboards_from_poses(self.level.game, &poses, dims);
            if self.cfg.render.debug.health_bars {
                bars = entities::health_bars_from_poses(self.level.game, &poses, dims);
            }
            self.level.map_dots = entities::map_dots_from_poses(
                self.level.game,
                &poses,
                &self.level.palette_rgba,
                self.cfg.render.enhancement.map_owned_buildings,
                self.level.mc2_env,
                // MC1 derives its ~4 Hz claimed-ball blink from the
                // tick; MC2's colorIndex_121 phases divide it.
                self.sim.tick as u32,
            );
            self.level.map_stamps = entities::map_stamps_from_poses(
                &poses,
                &self.level.map_icons,
                w.beyond_sight(),
                self.cfg.render.enhancement.expose_jar_spells,
            );
            self.jar_markers = if self.cfg.render.enhancement.expose_jar_spells {
                entities::jar_markers_from_poses(&poses)
            } else {
                Vec::new()
            };
            // Beyond-Sight rival position markers (interim for the
            // retail name labels — DrawText track).
            self.level.map_dots.extend(entities::rival_markers(
                &w.rival_views(),
                w.beyond_sight_tier(),
            ));
            self.castle_pos = poses
                .iter()
                .find(|p| p.class == 3 && p.model == 2 && p.player_owned)
                .map(|p| (p.x, p.z));
            self.level.map_areas = map_areas(w);
            // MC2 objective-guide targets (non-optional): the current
            // objective's live world targets → blinking marks + a steer
            // arrow. Empty off-MC2 (mc2_stages empty), so MC1/HW draw
            // nothing.
            self.level.objective_marks = w
                .mc2_objective_targets()
                .into_iter()
                .map(|t| mgc_render::ObjectiveMark {
                    x: t.x,
                    z: t.z,
                    nearest: t.nearest,
                    yellow: t.yellow,
                })
                .collect();
        }
        w.terrain_dirty = false;
        w.entities_dirty = false;
        let overlay = self.map_overlay();
        if let Some(r) = &mut self.renderer {
            if entities {
                r.set_billboards(self.level.billboards.clone());
                r.set_health_bars(bars);
            }
            // Upright map icons + the guide path are drawn screen-space
            // by the renderer (never baked into the rotated map
            // texture: icons stay upright, ant spacing stays 4 surface
            // px under rotation/zoom).
            r.set_map_stamps(self.level.map_stamps.clone());
            r.set_map_path(self.castle_pos.map(|(cx, cz)| mgc_render::MapPath {
                from: (self.sim.flyer.x, self.sim.flyer.z),
                to: (cx, cz),
                phase: (self.sim.tick & 3) as u8,
            }));
            // The objective-guide blink is tick-driven (retail gates:
            // outline 1-in-4, arrow 5-then-pause) — see
            // project_objective_marks.
            r.set_objective_marks(self.level.objective_marks.clone(), self.sim.tick as u32);
            if terrain {
                r.update_terrain(&self.level.view, &overlay);
                self.last_map_tick = Some(self.sim.tick);
            } else if self.last_map_tick != Some(self.sim.tick) {
                // The map recomposes once per SIM TICK — everything
                // baked in it (dots, blink phase tick>>3) changes at
                // tick rate, so per-frame recompose (a 256×256 LUT
                // walk + full texture upload) bought nothing. (The
                // marching ants march per frame regardless — they're
                // screen-space now. The old every-8th-tick throttle
                // was the player-reported low refresh; per-tick is
                // the content rate.)
                r.update_map(&self.level.view, &overlay);
                self.last_map_tick = Some(self.sim.tick);
            }
        }
    }

    /// Assemble the current baked map overlay: dots + the opt-in
    /// trigger circles. (Icon stamps and the guide path draw
    /// screen-space via `set_map_stamps`/`set_map_path`.)
    fn map_overlay(&self) -> mgc_render::MapOverlay {
        mgc_render::MapOverlay {
            dots: self.level.map_dots.clone(),
            areas: if self.cfg.render.debug.map_trigger_areas {
                self.level.map_areas.clone()
            } else {
                Vec::new()
            },
        }
    }

    /// While PAUSED the sim never consumes `pending_equip` (no ticks
    /// run), so the HUD hand icons wouldn't redraw until unpause —
    /// apply book bindings to the world immediately instead (binding
    /// is UI state, not simulation).
    fn flush_equip_if_paused(&mut self) {
        if !self.paused {
            return;
        }
        if let Some(w) = &mut self.sim.world {
            let l = self
                .pending_equip
                .0
                .take()
                .map(mgc_sim::mc1::spells::SpellId);
            let r = self
                .pending_equip
                .1
                .take()
                .map(mgc_sim::mc1::spells::SpellId);
            w.equip_hands(l, r);
            if let Some((spell, tier, hand)) = self.pending_mc2_select.take() {
                w.mc2_select_spell(spell, tier, hand);
            }
        }
    }

    /// The CTRL selector pane is up (hold-to-show; needs the pane
    /// surface enabled and UI sprites baked).
    fn pane_open(&self) -> bool {
        self.ctrl_held && self.pane.is_some() && self.level.ui.is_some()
    }

    fn pane_spell_name(&self, spell: u8) -> &'static str {
        if matches!(self.level.game, mgc_sim::ids::GameId::Mc2) {
            // The retail per-TIER hint name (docs/spell-audit/
            // spell-names.md): "Possession" / "Mana Magnet" / "Mana Lock"
            // by level, not one generic label. Resolves the live
            // hint_text so the Day/non-Day Morph/Army names come through.
            let tier = self.spell_levels[spell as usize] as usize;
            let name = self
                .sim
                .world
                .as_ref()
                .map(|w| w.mc2_spell_name(spell as usize, tier))
                .unwrap_or("");
            if name.is_empty() {
                ui::MC2_SPELL_NAMES[spell as usize]
            } else {
                name
            }
        } else {
            mgc_sim::mc1::spells::SpellId(spell).name()
        }
    }

    /// Commit a pane selection: persist the spell's chosen level (the
    /// original's `array_0x437[spell] = level`, every route reuses
    /// it) and bind the spell to the clicked hand.
    fn pane_commit(&mut self, slot: usize, hand: u8, level: u8) {
        let Some(pane) = &self.pane else { return };
        let spell = pane.order[slot];
        let multi = pane.levels > 1;
        self.spell_levels[spell as usize] = level;
        self.pane_bound[hand as usize] = Some(spell);
        let hand_name = if hand == 0 { "left" } else { "right" };
        if matches!(self.level.game, mgc_sim::ids::GameId::Mc2) {
            // The native MC2 spell column (Phase 4.2): the pane
            // commit IS retail's "Change Spell" action — tier +
            // quick-slot bind through the sim's class-15 machinery.
            self.pending_mc2_select = Some((spell, level, hand));
            self.flush_equip_if_paused();
            if multi {
                println!(
                    "selector: {hand_name} hand = {} level {}",
                    self.pane_spell_name(spell),
                    level + 1
                );
            } else {
                println!(
                    "selector: {hand_name} hand = {}",
                    self.pane_spell_name(spell)
                );
            }
            return;
        }
        // MC1: pane spell = the MC1 manifestation directly.
        if hand == 0 {
            self.pending_equip.0 = Some(spell);
        } else {
            self.pending_equip.1 = Some(spell);
        }
        self.flush_equip_if_paused();
        println!(
            "selector: {hand_name} hand = {}",
            self.pane_spell_name(spell)
        );
    }

    fn set_grab(&mut self, grab: bool) {
        let Some(window) = &self.window else { return };
        if grab {
            let ok = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
            window.set_cursor_visible(!ok);
            self.grabbed = ok;
        } else {
            window.set_cursor_grab(CursorGrabMode::None).ok();
            window.set_cursor_visible(true);
            self.grabbed = false;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // Default viewport = 1280×960 PHYSICAL px: exactly 2× the
        // native 640×480, so every UI/HUD element lands on an integer
        // pixel grid (no fractional-scale aliasing) and the aspect is
        // retail 4:3. Physical (not logical) so fractional DPI scales
        // (125% etc.) can't reintroduce a fractional multiple. The
        // window stays resizable; fullscreen/non-4:3 presentation is
        // the banked ui-native-layer work.
        let attrs = Window::default_attributes()
            .with_title(format!("Magic Carpet — {}", self.level.label))
            .with_inner_size(winit::dpi::PhysicalSize::new(1280u32, 960u32));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("error: cannot create window: {e}");
                event_loop.exit();
                return;
            }
        };
        match Renderer::for_window(window.clone()) {
            Ok(mut renderer) => {
                let overlay = self.map_overlay();
                renderer.load_level(&self.level.view, &overlay);
                if let Some((index, atlas)) = &self.level.sprites {
                    renderer.load_sprites(index.clone(), atlas);
                }
                if let Some(assets) = &self.level.ui {
                    renderer.load_ui_atlas(assets.atlas_w, assets.atlas_h, &assets.atlas_rgba);
                }
                renderer.set_billboards(self.level.billboards.clone());
                renderer.set_smooth_shading(self.cfg.render.enhancement.smooth_shading);
                renderer.set_hud_transparent(self.hud_transparent());
                if let Some(sky) = mc2_sky_srgb(&self.level) {
                    renderer.set_sky_color(sky);
                }
                // Map-screen topology follows the book surface: no
                // map book (MC2, or MC1 with spell_selector=mc2) =
                // the split layout with the stretched live view.
                renderer.set_map_layout(if self.selector.map_book {
                    mgc_render::MapScreenLayout::Mc1Book
                } else {
                    mgc_render::MapScreenLayout::Mc2Split
                });
                self.renderer = Some(renderer);
            }
            Err(e) => {
                eprintln!("error: renderer init: {e}");
                event_loop.exit();
                return;
            }
        }
        window.request_redraw();
        self.window = Some(window);
        self.last_frame = std::time::Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if self.pane_open() {
                    // The CTRL selector pane (over flight OR the map
                    // screen): press anchors the level flyout for the
                    // clicked hand, release commits level + binding
                    // (remc2 PI:806-929); SHIFT+click fast-binds the
                    // stored level (cmd 0x26). Fire never leaks
                    // through the pane.
                    let hand = match button {
                        MouseButton::Left => 0u8,
                        MouseButton::Right => 1u8,
                        _ => return,
                    };
                    if down {
                        if let Some(slot) = self.selector_hover.slot {
                            let spell = self.pane.as_ref().map(|p| p.order[slot]);
                            // Selectable = native-book ownership
                            // (MC2) / loadout ownership (MC1), or
                            // everything under the G instrument in
                            // MC2 (mirrors the pane view's grant).
                            let mc2 = matches!(self.level.game, mgc_sim::ids::GameId::Mc2);
                            let owned = (self.cfg.gameplay.cheat.dev_spells && mc2)
                                || spell
                                    .map(|c| {
                                        self.sim.world.as_ref().is_some_and(|w| {
                                            if mc2 {
                                                w.mc2_book_view().owned[c as usize]
                                            } else {
                                                w.loadout().owned[c as usize]
                                            }
                                        })
                                    })
                                    .unwrap_or(false);
                            if owned {
                                if self.shift_held {
                                    let level = self.spell_levels[spell.unwrap_or(0) as usize];
                                    self.pane_commit(slot, hand, level);
                                } else if self.selector_drag.is_none() {
                                    // A second button joining mid-drag
                                    // must not steal the live drag.
                                    self.selector_drag = Some((slot, hand));
                                }
                            }
                        }
                    } else if let Some((slot, h)) = self.selector_drag {
                        if h == hand {
                            let spell = self.pane.as_ref().map(|p| p.order[slot]).unwrap_or(0);
                            let level = self
                                .selector_hover
                                .level
                                .unwrap_or(self.spell_levels[spell as usize]);
                            self.pane_commit(slot, hand, level);
                            self.selector_drag = None;
                        }
                    }
                    self.fire_held = false;
                    self.fire_right_held = false;
                    return;
                }
                if self.book_open() {
                    // Book screen: clicking an owned spell binds it to
                    // that hand (the original's commands 0x15/0x16)
                    // AND closes the book back into flight (player-
                    // confirmed original UX). Clicks on unowned slots
                    // or empty page do nothing. (Without the map book
                    // — the MC2 layout — the map screen ignores
                    // clicks; the CTRL pane above is the selector.)
                    if down && self.selector.map_book {
                        let owned = self
                            .sim
                            .world
                            .as_ref()
                            .map(|w| w.loadout().owned)
                            .unwrap_or([false; 24]);
                        if let Some(spell) = self.hovered {
                            if owned[spell.0 as usize] {
                                match button {
                                    MouseButton::Left => self.pending_equip.0 = Some(spell.0),
                                    MouseButton::Right => self.pending_equip.1 = Some(spell.0),
                                    _ => return,
                                }
                                if let Some(r) = &mut self.renderer {
                                    r.set_map_view(false);
                                }
                                self.set_grab(true);
                                self.flush_equip_if_paused();
                            }
                        }
                    }
                    self.fire_held = false;
                    self.fire_right_held = false;
                    return;
                }
                if down && !self.grabbed {
                    self.set_grab(true);
                    return; // the grab click doesn't fire
                }
                match button {
                    MouseButton::Left => self.fire_held = down,
                    MouseButton::Right => self.fire_right_held = down,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::Focused(false) => {
                self.set_grab(false);
                self.fire_held = false;
                self.fire_right_held = false;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if down && event.logical_key == Key::Named(NamedKey::Escape) {
                    if self.grabbed {
                        self.set_grab(false);
                    } else {
                        event_loop.exit();
                    }
                    return;
                }
                if down && event.logical_key == Key::Named(NamedKey::Enter) {
                    if let Some(r) = &mut self.renderer {
                        let on = !r.map_view();
                        r.set_map_view(on);
                        // The screen-mode ding (sub_3DC90 :49072 —
                        // sound 14 on EVERY mode switch, enter and
                        // exit alike). While paused the request sits
                        // in the mixer and flushes on unpause — the
                        // retail deferred-ding quirk.
                        self.ui_ding();
                        // The book frees the cursor for spell binding;
                        // closing it returns to mouse-look.
                        if on {
                            self.set_grab(false);
                            self.fire_held = false;
                            self.fire_right_held = false;
                        } else {
                            self.set_grab(true);
                        }
                        // Entering/leaving the fullscreen map fixes
                        // your ORIENTATION but not your velocity in
                        // the original (player ground truth; traced
                        // as EMERGENT — map modes write no input, so
                        // the steering filters decay ~×0.75/tick to
                        // center while the target speed persists,
                        // :49017-20/:49044). We recenter the virtual
                        // stick; the sim's filters decay on their own
                        // because tick_input sends zero stick while
                        // the book is open.
                        self.stick = VirtualStick::default();
                    }
                    return;
                }
                // CTRL = the selector pane, hold-to-show / release-to-
                // close (remc2 keys[5]=0x1D, PI:505/PI:895). Opening
                // hijacks the pointer (grab off, OS cursor visible);
                // closing cancels any live drag and returns to
                // mouse-look unless the map screen keeps the cursor.
                if matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight)
                ) && self.pane.is_some()
                {
                    if down && !self.ctrl_held {
                        self.ctrl_held = true;
                        self.ctrl_grab_restore = self.grabbed;
                        if self.level.ui.is_some() {
                            self.set_grab(false);
                            self.fire_held = false;
                            self.fire_right_held = false;
                        }
                    } else if !down && self.ctrl_held {
                        self.ctrl_held = false;
                        self.selector_drag = None;
                        self.selector_hover = ui::SelectorHover::default();
                        if !self.book_open() && self.ctrl_grab_restore {
                            self.set_grab(true);
                        }
                    }
                    return;
                }
                // Quick keys 1..9,0 (enhancement; the original's only
                // digit path is the Ctrl+]+digit chord, :20340-56):
                // in the book, bind the hovered spell to the digit;
                // in flight, equip the bound spell (Shift = right hand).
                if down {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        let digit = match code {
                            KeyCode::Digit1 => Some(0),
                            KeyCode::Digit2 => Some(1),
                            KeyCode::Digit3 => Some(2),
                            KeyCode::Digit4 => Some(3),
                            KeyCode::Digit5 => Some(4),
                            KeyCode::Digit6 => Some(5),
                            KeyCode::Digit7 => Some(6),
                            KeyCode::Digit8 => Some(7),
                            KeyCode::Digit9 => Some(8),
                            KeyCode::Digit0 => Some(9),
                            _ => None,
                        };
                        if let Some(d) = digit {
                            if self.book_open() {
                                if let Some(spell) = self.hovered {
                                    // One spell ↔ one digit (retail:
                                    // assigning a quick key unassigns
                                    // the spell's previous one) — two
                                    // slots holding the same spell
                                    // would fight over the book's
                                    // digit badge.
                                    for b in self.quick_binds.iter_mut() {
                                        if *b == Some(spell.0) {
                                            *b = None;
                                        }
                                    }
                                    self.quick_binds[d] = Some(spell.0);
                                    println!("quick key {}: {}", (d + 1) % 10, spell.name());
                                }
                            } else if let Some(spell) = self.quick_binds[d] {
                                if self.shift_held {
                                    self.pending_equip.1 = Some(spell);
                                } else {
                                    self.pending_equip.0 = Some(spell);
                                }
                                self.flush_equip_if_paused();
                            }
                            return;
                        }
                    }
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::F1) {
                    // The original's sound toggle (remc1 :20086).
                    self.cfg.audio.sound = !self.cfg.audio.sound;
                    if let Some(a) = &mut self.audio {
                        a.set_volumes(
                            if self.cfg.audio.sound {
                                self.cfg.audio.sfx_volume
                            } else {
                                0.0
                            },
                            if self.cfg.audio.music {
                                self.cfg.audio.music_volume
                            } else {
                                0.0
                            },
                        );
                    }
                    println!(
                        "sound: {}{}",
                        if self.cfg.audio.sound { "on" } else { "off" },
                        if self.audio.is_none() {
                            " (no audio device)"
                        } else {
                            ""
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::F2) {
                    // The original's music toggle (remc1 :20100).
                    self.cfg.audio.music = !self.cfg.audio.music;
                    if let Some(a) = &mut self.audio {
                        if self.cfg.audio.music {
                            if let Some(track) = &self.level.music_track {
                                let _ = a.play_music(track, true);
                            }
                            a.set_volumes(
                                if self.cfg.audio.sound {
                                    self.cfg.audio.sfx_volume
                                } else {
                                    0.0
                                },
                                self.cfg.audio.music_volume,
                            );
                        } else {
                            a.stop_music();
                        }
                    }
                    println!(
                        "music: {}{}",
                        if self.cfg.audio.music { "on" } else { "off" },
                        if self.audio.is_none() {
                            " (no audio device)"
                        } else {
                            ""
                        }
                    );
                    return;
                }
                // Pause (retail P, drawing PAUSED at 132,50): the sim
                // clock freezes, the renderer/UI stay live — the quiet
                // room for inspecting the HUD/book without mobs.
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyP) {
                    self.paused = !self.paused;
                    // Retail pause suspends ALL sound (playtest-8);
                    // resumed sounds pick up where they froze.
                    if let Some(a) = &mut self.audio {
                        a.set_paused(self.paused);
                    }
                    println!("{}", if self.paused { "paused" } else { "unpaused" });
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyT) {
                    self.cfg.render.enhancement.smooth_shading =
                        !self.cfg.render.enhancement.smooth_shading;
                    if let Some(r) = &mut self.renderer {
                        r.set_smooth_shading(self.cfg.render.enhancement.smooth_shading);
                    }
                    println!(
                        "shading: {}",
                        if self.cfg.render.enhancement.smooth_shading {
                            "smooth (enhanced)"
                        } else {
                            "per-tile (original)"
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyV) {
                    self.cfg.render.debug.map_trigger_areas =
                        !self.cfg.render.debug.map_trigger_areas;
                    let overlay = self.map_overlay();
                    if let Some(r) = &mut self.renderer {
                        r.update_map(&self.level.view, &overlay);
                    }
                    println!(
                        "map trigger overlay: {}",
                        if self.cfg.render.debug.map_trigger_areas {
                            "on (enhanced)"
                        } else {
                            "off (original)"
                        }
                    );
                    return;
                }
                // Radar zoom (`+`/`-`, main row or numpad): tighten or
                // widen the in-flight minimap's world span. Faithful to
                // MC2's runtime radar zoom (likely MC1 too). The book
                // map always shows the whole world and is unaffected.
                if down {
                    let zoom = match event.physical_key {
                        PhysicalKey::Code(KeyCode::Equal | KeyCode::NumpadAdd) => Some(0.8),
                        PhysicalKey::Code(KeyCode::Minus | KeyCode::NumpadSubtract) => Some(1.25),
                        _ => None,
                    };
                    if let Some(factor) = zoom {
                        if let Some(r) = &mut self.renderer {
                            r.zoom_minimap(factor);
                            println!("radar zoom: {:.0} tiles", r.minimap_zoom());
                        }
                        return;
                    }
                }
                // The demolish key (MC1 Shift+L, scancode 0x26 under
                // the shift branch :20496-501): razes the OWN castle
                // one level per press — the castle-as-attack-spell
                // enabler, at the price of the respawn point.
                if down && self.shift_held && event.physical_key == PhysicalKey::Code(KeyCode::KeyL)
                {
                    self.pending_demolish = true;
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyG) {
                    self.cfg.gameplay.cheat.dev_spells = !self.cfg.gameplay.cheat.dev_spells;
                    if let Some(w) = &mut self.sim.world {
                        w.set_dev_spells(self.cfg.gameplay.cheat.dev_spells);
                    }
                    println!(
                        "dev spells: {}",
                        if self.cfg.gameplay.cheat.dev_spells {
                            "on — all spells, infinite mana (playtest instrument)"
                        } else {
                            "off (authentic acquisition/mana)"
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyH) {
                    self.cfg.render.debug.health_bars = !self.cfg.render.debug.health_bars;
                    if !self.cfg.render.debug.health_bars {
                        if let Some(r) = &mut self.renderer {
                            r.set_health_bars(Vec::new());
                        }
                    }
                    // On: bars appear with the next entity sync (every
                    // tick while creatures move).
                    println!(
                        "monster health bars: {}",
                        if self.cfg.render.debug.health_bars {
                            "on (debug enhancement)"
                        } else {
                            "off (original)"
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyC) {
                    self.cfg.render.debug.crosshair = !self.cfg.render.debug.crosshair;
                    println!(
                        "autoaim crosshair: {}",
                        if self.cfg.render.debug.crosshair {
                            "on (predictor instrument)"
                        } else {
                            "off (original)"
                        }
                    );
                    return;
                }
                let wasd = self.cfg.controls.preferences.bindings == config::Bindings::Wasd;
                let k = &mut self.keys;
                match event.physical_key {
                    // Thrust/strafe keys by binding profile. Classic =
                    // the original scheme (mouse aims, Up/Down arrows
                    // accelerate/decelerate, Left/Right strafe); the
                    // WASD profile keeps the arrows as enhanced-model
                    // turn/pitch keys.
                    PhysicalKey::Code(KeyCode::KeyW) if wasd => k.forward = down,
                    PhysicalKey::Code(KeyCode::KeyS) if wasd => k.back = down,
                    PhysicalKey::Code(KeyCode::KeyA) if wasd => k.left = down,
                    PhysicalKey::Code(KeyCode::KeyD) if wasd => k.right = down,
                    PhysicalKey::Code(KeyCode::ArrowUp) if !wasd => k.forward = down,
                    PhysicalKey::Code(KeyCode::ArrowDown) if !wasd => k.back = down,
                    PhysicalKey::Code(KeyCode::ArrowLeft) if !wasd => k.left = down,
                    PhysicalKey::Code(KeyCode::ArrowRight) if !wasd => k.right = down,
                    PhysicalKey::Code(KeyCode::ArrowLeft) => k.turn_left = down,
                    PhysicalKey::Code(KeyCode::ArrowRight) => k.turn_right = down,
                    PhysicalKey::Code(KeyCode::ArrowUp) => k.pitch_up = down,
                    PhysicalKey::Code(KeyCode::ArrowDown) => k.pitch_down = down,
                    // Extended-lift float moved to E/Q (2026-07-07,
                    // player directive): Space is the original's
                    // respawn/continue key and Shift now composes
                    // freely (Shift+L demolish, Shift+digit equips).
                    PhysicalKey::Code(KeyCode::KeyE) => k.up = down,
                    PhysicalKey::Code(KeyCode::KeyQ) => k.down = down,
                    PhysicalKey::Code(KeyCode::Space) => {
                        if down {
                            self.pending_respawn = true;
                        }
                    }
                    // Backspace = the retail MC2 full stop (action
                    // 0x27): speeds zero, Speed spell dies, steering
                    // recenters. Enhancement-class in MC1/HW (player
                    // directive 2026-07-16). The stick reset is
                    // retail's SetCenterScreenForFlyAssistant mouse
                    // recenter (EF:37965 → EF:44387).
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        if down {
                            self.pending_full_stop = true;
                            self.stick = VirtualStick::default();
                        }
                    }
                    PhysicalKey::Code(KeyCode::ShiftLeft) => {
                        self.shift_held = down;
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                // Clamp huge pauses (debugger, suspend) to keep the sim
                // from spiraling through hundreds of catch-up ticks.
                let dt = (now - self.last_frame).as_secs_f32().min(0.25);
                self.last_frame = now;
                self.accumulator += dt;
                if self.paused {
                    // Frozen sim clock: drain the accumulator so
                    // unpausing resumes cleanly instead of bursting
                    // through the missed ticks.
                    self.accumulator = 0.0;
                }

                while self.accumulator >= TICK_DT {
                    self.accumulator -= TICK_DT;
                    self.prev_flyer = self.sim.flyer;
                    let input = self.tick_input();
                    self.sim.step(&input);
                    // The mixer flush is per-tick like the original's
                    // (fade ramps are tick-denominated).
                    self.audio_tick();
                }
                // Limit-removing telemetry (ROADMAP "MULTI-GAME
                // ARCHITECTURE"): the pool fails open like retail,
                // but every dropped spawn is worth a report — this
                // is how the catalogue of ceiling-hitting levels
                // (032's starved trigger, 039's walls) gets built.
                if let Some(w) = self.sim.world.as_mut() {
                    // Retail quickselect auto-assign (:64858-67): a
                    // newly acquired spell takes the FIRST FREE quick
                    // key (scan 1→9→0, cap 10, silent when full;
                    // already-bound spells never re-assign). Walking
                    // the book's canonical order (byte_99B88) also
                    // reproduces the level-init pre-seed (:49216-59):
                    // at level start every owned spell diffs in at
                    // once, in that order. MC1-key schemes only —
                    // MC2 controls have no quickselect bank.
                    if self.selector.map_book {
                        let owned = w.loadout().owned;
                        for &s in &SPELL_CANON {
                            let s = s as usize;
                            if owned[s]
                                && !self.prev_owned[s]
                                && !self.quick_binds.contains(&Some(s as u8))
                            {
                                if let Some(slot) =
                                    self.quick_binds.iter_mut().find(|b| b.is_none())
                                {
                                    *slot = Some(s as u8);
                                }
                            }
                        }
                        self.prev_owned = owned;
                    }
                    let dropped = w.take_pool_exhausted();
                    if dropped > 0 {
                        self.pool_dropped_total += dropped;
                        println!(
                            "ERROR: entity pool exhausted — {dropped} allocation(s) \
                             dropped this frame, {} this level (fail-open, as retail)",
                            self.pool_dropped_total
                        );
                    }
                    // The spawn seam's misfit ledger (unknown
                    // (class, model) things degraded gracefully) —
                    // report new entries once.
                    for &(class, model, count) in &w.misfits()[self.misfits_reported..] {
                        println!(
                            "WARN: misfit thing (class {class}, model {model}) x{count} — \
                             unknown to the serving spawn column, degraded"
                        );
                        self.misfits_reported += 1;
                    }
                }
                self.sync_world();
                // Castle-less death confirmed → the level restarts
                // (the original's lost + level-over flow).
                if self.sim.world.as_mut().is_some_and(|w| w.take_restart()) {
                    self.restart_level();
                }

                let alpha = self.accumulator / TICK_DT;
                let (a, b) = (&self.prev_flyer, &self.sim.flyer);
                // Positions may wrap across the 256-tile seam; take the
                // short way around for interpolation.
                let lerp_wrap = |p: f32, q: f32| {
                    let mut d = q - p;
                    if d > 128.0 {
                        d -= 256.0;
                    }
                    if d < -128.0 {
                        d += 256.0;
                    }
                    (p + d * alpha).rem_euclid(256.0)
                };
                // The knock camera kick (remc1 :52433-37): the view
                // pitches down ~v_22/8 engine-angle units while a
                // buffet/knock is live (the kraken drag feedback).
                let kick = self
                    .sim
                    .world
                    .as_ref()
                    .map(|w| w.knock_magnitude() as f32 / 8.0 * (std::f32::consts::TAU / 2048.0))
                    .unwrap_or(0.0);
                // The faithful camera renders at HALF the aim pitch
                // (remc1 :52434: pitch_8 = u16_329/2) — casts still
                // aim along the full published pitch.
                let aim = a.pitch + (b.pitch - a.pitch) * alpha;
                // Faithful only: the horizon bank from the filtered
                // roll stick, full value (remc1 :52432 — the missing
                // turn cue). The enhanced mouse-look stays flat by
                // player directive.
                let (view_pitch, view_roll) = match self.cfg.controls.models.thrust {
                    config::ThrustModel::Mc1 => (aim * 0.5, a.roll + (b.roll - a.roll) * alpha),
                    config::ThrustModel::Enhanced => (aim, 0.0),
                };
                let cam = CameraView {
                    x: lerp_wrap(a.x, b.x),
                    y: a.y + (b.y - a.y) * alpha,
                    z: lerp_wrap(a.z, b.z),
                    yaw: a.yaw + (b.yaw - a.yaw) * alpha,
                    pitch: view_pitch - kick,
                    roll: view_roll,
                    fov_y: FOV_Y,
                };
                // Spell UI quads (book grid or in-flight HUD).
                if let (Some(assets), Some(w)) = (&self.level.ui, &self.sim.world) {
                    let size = self
                        .window
                        .as_ref()
                        .map(|win| win.inner_size())
                        .map(|s| (s.width as f32, s.height as f32))
                        .unwrap_or((1280.0, 960.0));
                    let loadout = w.loadout();
                    let vitals = w.vitals();
                    let mc2_book = matches!(self.level.game, mgc_sim::ids::GameId::Mc2)
                        .then(|| w.mc2_book_view());
                    // The alert-marble flicker approximates retail's
                    // per-frame [55]/[41] alternation at tick parity.
                    let alert_blink = self.sim.tick % 2 == 0;
                    let (mut quads, hovered) = if self.book_open() {
                        if self.selector.map_book {
                            ui::book_quads(
                                assets,
                                &loadout,
                                &self.quick_binds,
                                size.0,
                                size.1,
                                self.cursor,
                            )
                        } else {
                            // The MC2-layout map screen has no book
                            // half — the renderer's split layout shows
                            // the stretched live view there; the CTRL
                            // pane below is the selector.
                            (Vec::new(), None)
                        }
                    } else {
                        (
                            ui::hud_quads(
                                assets,
                                &loadout,
                                &vitals,
                                self.hud_transparent(),
                                alert_blink,
                                matches!(self.level.game, mgc_sim::ids::GameId::Mc2),
                                mc2_book.as_ref(),
                                size.0,
                                size.1,
                            ),
                            None,
                        )
                    };
                    // The CTRL selector pane, over flight or the map
                    // screen alike (the original draws the same pane
                    // in both states, remc2 EF:21788/EF:21959).
                    if self.pane_open() {
                        if let Some(pane) = &self.pane {
                            let n = pane.spell_count();
                            let mc2 = matches!(self.level.game, mgc_sim::ids::GameId::Mc2);
                            let mut owned = [false; 26];
                            let mut castable = [false; 26];
                            let mut cost = [0u32; 26];
                            let mut max_level = [0u8; 26];
                            let mut sel = [0u8; 26];
                            let mut xp = [0i32; 26];
                            let mut xpos = [[0i32; 3]; 26];
                            let mut bound = [loadout.left, loadout.right];
                            if mc2 {
                                // The native spell book (Phase 4.2):
                                // ownership, per-spell LEVEL (the
                                // SpellLevels tier ceiling), selected
                                // tiers, real GetSpellManaCost costs
                                // and the quick-slot binds all come
                                // from the sim's class-15 machinery.
                                let bv = self.sim.world.as_ref().map(|w| w.mc2_book_view());
                                if let Some(bv) = bv {
                                    for s in 0..n {
                                        owned[s] =
                                            bv.owned[s] || self.cfg.gameplay.cheat.dev_spells;
                                        castable[s] = owned[s];
                                        cost[s] = bv.cost[s];
                                        // The G instrument keeps all
                                        // tiers exercisable (player
                                        // 2026-07-10); the earned
                                        // ceiling is the XP level.
                                        max_level[s] = if self.cfg.gameplay.cheat.dev_spells {
                                            pane.levels - 1
                                        } else {
                                            bv.levels[s]
                                        };
                                        sel[s] = bv.sel[s];
                                        xp[s] = bv.xp[s];
                                        xpos[s] = bv.xpos[s];
                                    }
                                    bound =
                                        [u8::try_from(bv.left).ok(), u8::try_from(bv.right).ok()];
                                    // Mirror for the drag/commit path.
                                    self.spell_levels = sel;
                                    self.pane_bound = bound;
                                }
                            } else {
                                for s in 0..n {
                                    owned[s] = loadout.owned[s];
                                    castable[s] = loadout.bindable[s];
                                    cost[s] = mgc_sim::mc1::spells::SPELLS[s].possess_mana;
                                    max_level[s] = pane.levels - 1;
                                    sel[s] = self.spell_levels[s];
                                }
                            }
                            let view = ui::SelectorView {
                                owned: &owned[..n],
                                castable: &castable[..n],
                                selected_level: &sel[..n],
                                max_level: &max_level[..n],
                                bound,
                                mana: loadout.mana,
                                cost: &cost[..n],
                                xp: &xp[..n],
                                xpos: &xpos[..n],
                            };
                            let (pq, hover) = ui::selector_quads(
                                assets,
                                pane,
                                &view,
                                size.0,
                                size.1,
                                self.cursor,
                                self.selector_drag.map(|(s, _)| s),
                            );
                            quads.extend(pq);
                            self.selector_hover = hover;
                        }
                    }
                    if !self.book_open() {
                        quads.extend(ui::vitals_quads(
                            &vitals,
                            size.0,
                            size.1,
                            (self.sim.tick / 8) % 2 == 0,
                            self.cfg.render.debug.grace_meter,
                        ));
                    }
                    if self.paused {
                        // Both views: the book screen is exactly where
                        // paused inspection happens.
                        quads.extend(ui::pause_quads(size.0, size.1));
                    }
                    // expose-jar-spells (debug): float each pickable
                    // jar's spell icon over it in the main view (the
                    // map stamps are the other half). No fancy UI —
                    // the raw icon on a dark slab, health-bar style.
                    if self.cfg.render.enhancement.expose_jar_spells && !self.book_open() {
                        if let Some(u) = &self.level.ui {
                            for &(x, alt, z, spell) in &self.jar_markers {
                                let Some(id) = ui::spell_icon_sprite(self.level.game, spell) else {
                                    continue;
                                };
                                let Some(st) = u.map_stamp(id) else { continue };
                                let Some((sx, sy)) = mgc_render::world_to_screen(
                                    &cam,
                                    size.0,
                                    size.1,
                                    x,
                                    alt + 0.6,
                                    z,
                                ) else {
                                    continue;
                                };
                                let s = (size.0 / 640.0).max(1.0);
                                let ih = 12.0 * s;
                                let iw = ih * st.w as f32 / st.h as f32;
                                // A dark slab behind the luminous icon
                                // ramps, for readability over bright sky/
                                // terrain.
                                quads.push(mgc_render::UiQuad {
                                    rect: [
                                        sx - iw * 0.5 - s,
                                        sy - ih - s,
                                        iw + 2.0 * s,
                                        ih + 2.0 * s,
                                    ],
                                    uv: [0.0; 4],
                                    tint: [0.0, 0.0, 0.0, 0.45],
                                });
                                quads.push(mgc_render::UiQuad {
                                    rect: [sx - iw * 0.5, sy - ih, iw, ih],
                                    uv: st.uv,
                                    tint: [1.0, 1.0, 1.0, 1.0],
                                });
                            }
                        }
                    }
                    // The autoaim crosshair (P-class predictor;
                    // `render.debug.crosshair`, C toggles): the
                    // white-edged cross at the TRUE aim point (full
                    // aim pitch — the faithful camera runs half), and
                    // +/x lock markers on the target each hand's
                    // equipped spell would acquire this instant
                    // (World::aim_preview — the pure scan twin).
                    if self.cfg.render.debug.crosshair
                        && !self.book_open()
                        && vitals.state == mgc_sim::mc1::world::LifeState::Alive
                    {
                        let f = &self.sim.flyer;
                        let (sy, cyaw) = cam.yaw.sin_cos();
                        let (sp, cp) = aim.sin_cos();
                        // The acquire range: 5120 units = 20 tiles.
                        const AIM_D: f32 = 20.0;
                        let neutral = mgc_render::world_to_screen(
                            &cam,
                            size.0,
                            size.1,
                            cam.x + sy * cp * AIM_D,
                            cam.y + sp * AIM_D,
                            cam.z - cyaw * cp * AIM_D,
                        );
                        let pose = mgc_sim::mc1::world::PlayerPose::from_tiles(
                            f.x, f.y, f.z, f.yaw, f.pitch, 0.0,
                        );
                        let locks = w.aim_preview(pose).map(|l| {
                            l.and_then(|l| {
                                mgc_render::world_to_screen(&cam, size.0, size.1, l.x, l.alt, l.z)
                            })
                        });
                        let blink =
                            0.5 + 0.5 * (((self.sim.tick % 4096) as f32 + alpha) * 0.4).sin();
                        ui::crosshair_quads(&mut quads, size.0, neutral, locks, blink);
                    }
                    // The top-of-screen notification line (retail
                    // `DrawTextPauseEndOfLevel_2CE30`, EF:21787): the
                    // small FONT1 toast, LEFT-aligned, anchored just below
                    // the wizard info-boxes and right of the radar (the
                    // HUD-derived anchor — retail's 320-native literal
                    // doesn't map onto our 640-native HSPR panels). Over
                    // the live view only (not the book/map screen). The
                    // anchor is in 640-native HUD coords (× w/640); FONT1
                    // draws at gameUiScale, so its glyphs scale by w/320.
                    // The white masks are tinted the ink colour (DrawText's
                    // `color`, red for plain toasts).
                    if !self.book_open() && assets.has_font() {
                        if let Some((msg, color)) = w.notification() {
                            let (ax, ay) = assets.hud_notification_anchor();
                            let hud_s = size.0 / 640.0;
                            let font_s = size.0 / 320.0;
                            let tint = [
                                color[0] as f32 / 255.0,
                                color[1] as f32 / 255.0,
                                color[2] as f32 / 255.0,
                                1.0,
                            ];
                            quads.extend(assets.text_quads(
                                msg,
                                ax * hud_s,
                                ay * hud_s,
                                tint,
                                font_s,
                            ));
                        }
                        // The MC1/HW WIN message (:26480-26505):
                        // while the win flag holds, the two-line
                        // black-ink message persists at the pane top
                        // — ETEXT.DAT entries 60/61 (verified against
                        // the pristine install; the full etext bake
                        // is the banked Text track). Retail
                        // colour-cycles the ink unless zoomed out —
                        // the static black remap slot [1] is the
                        // baseline.
                        if !matches!(self.level.game, mgc_sim::ids::GameId::Mc2)
                            && w.completed()
                            && !w.player_dead()
                        {
                            let (ax, ay) = assets.hud_notification_anchor();
                            let hud_s = size.0 / 640.0;
                            let font_s = size.0 / 320.0;
                            let black = [0.0, 0.0, 0.0, 1.0];
                            // One string — the font's own line height
                            // spaces the two lines (the manual offset
                            // pass under-spaced and the lines
                            // overlapped, playtest 2026-07-16). A
                            // live toast owns the anchor row; the
                            // win block steps one line below it.
                            let msg = if w.notification().is_some() {
                                "\nWorld restored.\nPress the space bar to continue."
                            } else {
                                "World restored.\nPress the space bar to continue."
                            };
                            quads.extend(assets.text_quads(
                                msg,
                                ax * hud_s,
                                ay * hud_s,
                                black,
                                font_s,
                            ));
                        }
                    }
                    // The end-of-game fadeout: the MC2 ending's
                    // sim-side fade (endGameSeq phase 11) under the
                    // app's own post-victory fade; at full black the
                    // game ends (player directive 2026-07-16 — quit,
                    // no stats/menu; campaign stitching later).
                    if w.won() && self.quit_fade.is_none() {
                        // The victory breadcrumb (player request
                        // 2026-07-16) — the campaign-stitching hook
                        // will consume the same signal later.
                        println!("{} completed", self.level.label);
                        self.quit_fade = Some(0.0);
                    }
                    let fade = w.end_fade().max(self.quit_fade.unwrap_or(0.0));
                    if fade > 0.0 {
                        quads.push(ui::solid([0.0, 0.0, size.0, size.1], [0.0, 0.0, 0.0, fade]));
                    }
                    self.hovered = hovered;
                    if let Some(r) = &mut self.renderer {
                        r.set_ui_quads(quads);
                    }
                }
                if let Some(f) = &mut self.quit_fade {
                    *f += 1.0 / 48.0;
                    if *f >= 1.25 {
                        // A beat of full black before leaving.
                        event_loop.exit();
                    }
                }
                if let Some(r) = &mut self.renderer {
                    // Animation clock: sim ticks are the original's game
                    // turns; wrapped so f32 stays exact (see set_anim_turn).
                    r.set_anim_turn((self.sim.tick % 4096) as f32 + alpha);
                    match r.render(&cam) {
                        Ok(()) | Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {}
                        Err(e) => eprintln!("render: {e}"),
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if !self.grabbed {
            return;
        }
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            let dy = if self.cfg.controls.preferences.invert_y {
                -dy
            } else {
                dy
            };
            if self.cfg.controls.models.thrust == config::ThrustModel::Mc1 {
                // Relative motion integrates into the virtual stick
                // POSITION (the original reads the DOS cursor offset
                // from screen center, clamped ±127 — on a 320-wide
                // screen that's ~0.8 stick units per pixel; modern
                // default trades a little of that for precision).
                let s = STICK_PER_PIXEL * self.cfg.controls.preferences.mouse_sensitivity;
                self.stick.x = (self.stick.x + dx as f32 * s).clamp(-127.0, 127.0);
                self.stick.y = (self.stick.y - dy as f32 * s).clamp(-127.0, 127.0);
                self.stick_idle_ticks = 0;
            } else {
                let s = MOUSE_SENSITIVITY * self.cfg.controls.preferences.mouse_sensitivity;
                self.mouse.yaw += dx as f32 * s;
                self.mouse.pitch -= dy as f32 * s;
            }
        }
    }
}

struct Args {
    level: PathBuf,
    screenshot: Option<PathBuf>,
    /// Camera override for screenshots: x, y, z, yaw°, pitch°.
    camera: Option<[f32; 5]>,
    /// MC1 world tileset override: 0 = temperate, 1 = arctic.
    /// None = by game (mc1 temperate, mc1hw arctic).
    tileset: Option<u8>,
    /// Config file path; None = the default `mgcarpet.json` lookup.
    config: Option<PathBuf>,
    /// CLI override of `render.enhancement.smooth_shading`; None = use config.
    smooth_shading: Option<bool>,
    /// CLI override of `render.debug.map_trigger_areas`.
    map_triggers: Option<bool>,
    /// CLI override of `render.debug.health_bars`.
    health_bars: Option<bool>,
    crosshair: Option<bool>,
    /// CLI override of `gameplay.cheat.dev_spells`.
    dev_spells: Option<bool>,
    /// CLI override of `dev.plausible_spellbook`.
    plausible_spellbook: Option<bool>,
    /// CLI override of `gameplay.enhancement.prune_owned_jars`.
    prune_owned_jars: Option<bool>,
    /// CLI override of `gameplay.cheat.invincible`.
    invincible: Option<bool>,
    /// CLI override of `render.enhancement.expose_jar_spells`.
    expose_jar_spells: Option<bool>,
    /// CLI override of `render.debug.grace_meter`.
    grace_meter: Option<bool>,
    /// CLI overrides of the `flight` tier enums; None = use config.
    thrust: Option<config::ThrustModel>,
    altitude: Option<config::AltitudeModel>,
    bindings: Option<config::Bindings>,
    /// Write the overhead map as a PNG and exit (one pixel per tile,
    /// scaled by `map_scale`).
    map: Option<PathBuf>,
    map_scale: u32,
    /// Render `--screenshot` showing the book screen instead of the world.
    map_view: bool,
    /// Spell-selector surface override (config `spell_selector`).
    spell_selector: Option<config::SpellSelector>,
    /// Animation clock for `--screenshot` (game turns; default 0).
    /// Water-wave phase repeats every 32 (MC1) / 64 (MC2) turns.
    anim_turn: f32,
    /// Apply the original's load-time terrain features (default true).
    terrain_features: bool,
    /// Entity-pool size override (limit-removing dev flag, G-class);
    /// None = the game's pristine chassis value (1000).
    pool_slots: Option<usize>,
    awake_range: Option<u32>,
    /// Headless flocking probe: tick the real world and dump per-
    /// creature AI state as CSV (the goat-cohesion diagnostic).
    flock_probe: Option<PathBuf>,
    probe_ticks: u32,
    /// CSV row cadence (1 = every tick).
    probe_every: u32,
    /// Pose script: far|start|hover[:ALT]|approach[:ALT]|orbit[:ALT].
    probe_pose: String,
    /// Tracked (class, model); default (5,1) = the MC2 goat.
    probe_species: (u8, u8),
    /// Minimal environment: landscape + the tracked species only.
    probe_strip: bool,
    /// Dispositions fired at t=0 (materialize dis-gated spawns —
    /// e.g. mc2:00's dis-6 quest fireflies).
    probe_dis: Vec<u16>,
}

fn parse_args() -> Result<Args, String> {
    let mut level = PathBuf::from("baked/mc1/level-000.mgcl");
    let mut screenshot = None;
    let mut camera = None;
    let mut tileset = None;
    let mut config = None;
    let mut smooth_shading = None;
    let mut map_triggers = None;
    let mut health_bars = None;
    let mut crosshair = None;
    let mut dev_spells = None;
    let mut plausible_spellbook = None;
    let mut prune_owned_jars = None;
    let mut invincible = None;
    let mut expose_jar_spells = None;
    let mut grace_meter = None;
    let mut thrust = None;
    let mut altitude = None;
    let mut bindings = None;
    let mut map = None;
    let mut map_scale = 4u32;
    let mut map_view = false;
    let mut spell_selector = None;
    let mut anim_turn = 0.0f32;
    let mut terrain_features = true;
    let mut awake_range = None;
    let mut pool_slots = None;
    let mut flock_probe = None;
    let mut probe_ticks = 8000u32;
    let mut probe_every = 1u32;
    let mut probe_pose = String::from("start");
    let mut probe_species = (5u8, 1u8);
    let mut probe_strip = false;
    let mut probe_dis = Vec::new();

    /// `--level` accepts a package path or the path-free shorthand
    /// `<game>:<index>` (`mc1:32`, `mc1hw:7`, `mc2:100`) resolving to
    /// `baked/<game>/level-NNN.mgcl` — typeable before the baked tree
    /// exists, when there is no file to tab-complete (the launch
    /// itself bakes it). Anything not starting with a known game tag
    /// is a path (Windows drive prefixes like `C:` fall through).
    fn resolve_level_arg(spec: &str) -> Result<PathBuf, String> {
        match spec.split_once(':') {
            Some((game @ ("mc1" | "mc1hw" | "mc2"), index)) => {
                let index: u32 = index
                    .parse()
                    .map_err(|e| format!("--level {spec}: bad level index: {e}"))?;
                Ok(PathBuf::from(format!("baked/{game}/level-{index:03}.mgcl")))
            }
            // A numeric index after an unknown tag is a typo'd
            // shorthand, not a path — fail fast instead of hunting
            // (and baking) for a file literally named `mc3:5`.
            Some((game, index)) if index.parse::<u32>().is_ok() => Err(format!(
                "--level {spec}: unknown game {game:?} (mc1, mc1hw or mc2)"
            )),
            _ => Ok(PathBuf::from(spec)),
        }
    }

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--level" => {
                level = resolve_level_arg(&it.next().ok_or("--level needs a path or game:index")?)?;
            }
            "--tileset" => {
                let set: u8 = it
                    .next()
                    .ok_or("--tileset needs 0 or 1")?
                    .parse()
                    .map_err(|e| format!("--tileset: {e}"))?;
                if set > 1 {
                    return Err("--tileset must be 0 (temperate) or 1 (arctic)".into());
                }
                tileset = Some(set);
            }
            "--screenshot" => {
                screenshot = Some(PathBuf::from(it.next().ok_or("--screenshot needs a path")?));
            }
            "--flock-probe" => {
                flock_probe = Some(PathBuf::from(
                    it.next().ok_or("--flock-probe needs a csv path")?,
                ));
            }
            "--probe-ticks" => {
                probe_ticks = it
                    .next()
                    .ok_or("--probe-ticks needs a count")?
                    .parse()
                    .map_err(|e| format!("--probe-ticks: {e}"))?;
            }
            "--probe-every" => {
                probe_every = it
                    .next()
                    .ok_or("--probe-every needs a tick interval")?
                    .parse::<u32>()
                    .map_err(|e| format!("--probe-every: {e}"))?
                    .max(1);
            }
            "--probe-pose" => {
                probe_pose = it
                    .next()
                    .ok_or("--probe-pose needs far|start|hover[:ALT]|approach[:ALT]|orbit[:ALT]")?;
            }
            "--probe-species" => {
                let spec = it.next().ok_or("--probe-species needs class,model")?;
                let (c, m) = spec
                    .split_once(',')
                    .ok_or_else(|| format!("--probe-species {spec}: expected class,model"))?;
                probe_species = (
                    c.parse().map_err(|e| format!("--probe-species: {e}"))?,
                    m.parse().map_err(|e| format!("--probe-species: {e}"))?,
                );
            }
            "--probe-strip" => probe_strip = true,
            "--probe-dis" => {
                let spec = it
                    .next()
                    .ok_or("--probe-dis needs dis ids (comma-separated)")?;
                for part in spec.split(',') {
                    probe_dis.push(part.parse().map_err(|e| format!("--probe-dis: {e}"))?);
                }
            }
            "--camera" => {
                let spec = it.next().ok_or("--camera needs x,y,z,yaw,pitch")?;
                let vals: Vec<f32> = spec
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect::<Result<_, _>>()
                    .map_err(|e| format!("--camera: {e}"))?;
                camera = Some(
                    vals.try_into()
                        .map_err(|_| "--camera needs exactly 5 values".to_string())?,
                );
            }
            "--config" => {
                config = Some(PathBuf::from(it.next().ok_or("--config needs a path")?));
            }
            "--smooth-shading" => smooth_shading = Some(true),
            "--no-smooth-shading" => smooth_shading = Some(false),
            "--map-triggers" => map_triggers = Some(true),
            "--no-map-triggers" => map_triggers = Some(false),
            "--health-bars" => health_bars = Some(true),
            "--no-health-bars" => health_bars = Some(false),
            "--crosshair" => crosshair = Some(true),
            "--no-crosshair" => crosshair = Some(false),
            "--dev-spells" => dev_spells = Some(true),
            "--no-dev-spells" => dev_spells = Some(false),
            "--plausible-spellbook" => plausible_spellbook = Some(true),
            "--no-plausible-spellbook" => plausible_spellbook = Some(false),
            "--prune-owned-jars" => prune_owned_jars = Some(true),
            "--no-prune-owned-jars" => prune_owned_jars = Some(false),
            "--invincible" => invincible = Some(true),
            "--no-invincible" => invincible = Some(false),
            "--expose-jar-spells" => expose_jar_spells = Some(true),
            "--no-expose-jar-spells" => expose_jar_spells = Some(false),
            "--grace-meter" => grace_meter = Some(true),
            "--no-grace-meter" => grace_meter = Some(false),
            "--thrust" => {
                thrust = Some(match it.next().as_deref() {
                    Some("mc1") => config::ThrustModel::Mc1,
                    Some("enhanced") => config::ThrustModel::Enhanced,
                    _ => return Err("--thrust needs mc1|enhanced".into()),
                });
            }
            "--altitude" => {
                altitude = Some(match it.next().as_deref() {
                    Some("faithful") => config::AltitudeModel::Faithful,
                    Some("extended-lift") => config::AltitudeModel::ExtendedLift,
                    _ => return Err("--altitude needs faithful|extended-lift".into()),
                });
            }
            "--bindings" => {
                bindings = Some(match it.next().as_deref() {
                    Some("classic") => config::Bindings::Classic,
                    Some("wasd") => config::Bindings::Wasd,
                    _ => return Err("--bindings needs classic|wasd".into()),
                });
            }
            "--map" => {
                map = Some(PathBuf::from(it.next().ok_or("--map needs a path")?));
            }
            "--map-scale" => {
                map_scale = it
                    .next()
                    .ok_or("--map-scale needs a factor")?
                    .parse()
                    .map_err(|e| format!("--map-scale: {e}"))?;
                if map_scale == 0 || map_scale > 16 {
                    return Err("--map-scale must be 1..=16".into());
                }
            }
            "--map-view" => map_view = true,
            "--spell-selector" => {
                spell_selector = Some(match it.next().as_deref() {
                    Some("auto") => config::SpellSelector::Auto,
                    Some("mc1") => config::SpellSelector::Mc1,
                    Some("mc2") => config::SpellSelector::Mc2,
                    Some("mc1+mc2") => config::SpellSelector::Mc1Mc2,
                    _ => return Err("--spell-selector needs auto|mc1|mc2|mc1+mc2".into()),
                });
            }
            "--anim-turn" => {
                anim_turn = it
                    .next()
                    .ok_or("--anim-turn needs a turn count")?
                    .parse()
                    .map_err(|e| format!("--anim-turn: {e}"))?;
            }
            "--no-terrain-features" => terrain_features = false,
            "--pool-slots" => {
                let n: usize = it
                    .next()
                    .ok_or("--pool-slots needs a count")?
                    .parse()
                    .map_err(|e| format!("--pool-slots: {e}"))?;
                if !(2..=60000).contains(&n) {
                    return Err("--pool-slots must be in 2..=60000 (slots are u16)".into());
                }
                pool_slots = Some(n);
            }
            "--awake-range" => {
                let n: u32 = it
                    .next()
                    .ok_or("--awake-range needs a tile count (0 = always awake)")?
                    .parse()
                    .map_err(|e| format!("--awake-range: {e}"))?;
                awake_range = Some(n);
            }
            "--help" | "-h" => {
                return Err(format!(
                    "usage: mgcarpet [--level <game:index> | <baked/.../level-NNN.mgcl>] \
                     [--tileset 0|1] [--config <path>] \
                     [--smooth-shading|--no-smooth-shading] \
                     [--map-triggers|--no-map-triggers] \
                     [--crosshair|--no-crosshair] \
                     [--health-bars|--no-health-bars] \
                     [--dev-spells|--no-dev-spells] \
                     [--plausible-spellbook|--no-plausible-spellbook] \
                     [--prune-owned-jars|--no-prune-owned-jars] \
                     [--invincible|--no-invincible] \
                     [--expose-jar-spells|--no-expose-jar-spells] \
                     [--grace-meter|--no-grace-meter] \
                     [--thrust mc1|enhanced] [--altitude faithful|extended-lift] \
                     [--bindings classic|wasd] \
                     [--spell-selector auto|mc1|mc2|mc1+mc2] \
                     [--screenshot out.png [--camera x,y,z,yaw,pitch] [--map-view] \
                     [--anim-turn N]] \
                     [--map out.png [--map-scale N]] [--no-terrain-features] \
                     [--pool-slots N] [--awake-range TILES (0 = always awake)] \
                     [--flock-probe out.csv [--probe-ticks N] [--probe-every N] \
                     [--probe-pose far|start|hover[:ALT]|approach[:ALT]|orbit[:ALT]] \
                     [--probe-species CLASS,MODEL] [--probe-strip] [--probe-dis N,N..]]\n\
                     enhancements persist in {} (see crates/mgc-app/src/config.rs)",
                    config::DEFAULT_PATH
                ));
            }
            other => return Err(format!("unknown argument {other} (try --help)")),
        }
    }
    Ok(Args {
        level,
        screenshot,
        camera,
        tileset,
        config,
        smooth_shading,
        map_triggers,
        health_bars,
        crosshair,
        dev_spells,
        plausible_spellbook,
        prune_owned_jars,
        invincible,
        expose_jar_spells,
        grace_meter,
        thrust,
        altitude,
        bindings,
        map,
        map_scale,
        map_view,
        spell_selector,
        anim_turn,
        terrain_features,
        pool_slots,
        awake_range,
        flock_probe,
        probe_ticks,
        probe_every,
        probe_pose,
        probe_species,
        probe_strip,
        probe_dis,
    })
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write the overhead map (one pixel per tile through the engine's
/// map-color path), nearest-neighbor scaled — the axis-aligned,
/// rotation-free comparison artifact for original map screenshots.
fn run_map(level: &LoadedLevel, out: &Path, scale: u32, map_triggers: bool) -> Result<(), String> {
    let n = 256usize;
    // Stamps/path are screen-space projected at render time; this raw
    // CPU dump (the diagnostic artifact) shows dots only.
    let overlay = mgc_render::MapOverlay {
        dots: level.map_dots.clone(),
        areas: if map_triggers {
            level.map_areas.clone()
        } else {
            Vec::new()
        },
    };
    let src = mgc_render::map_pixels(&level.view, &overlay);
    let s = scale as usize;
    let (w, h) = (n * s, n * s);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let si = ((y / s) * n + x / s) * 4;
            let di = (y * w + x) * 4;
            rgba[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    write_png(out, w as u32, h as u32, &rgba)?;
    println!("{} -> {} ({}x{})", level.label, out.display(), w, h);
    Ok(())
}

/// Headless flocking probe (`--flock-probe`, the goat-cohesion
/// mystery): tick the REAL app world (the same `WorldInit::build` the
/// game plays on — not the sim-test fixture) and dump every tracked
/// creature's full AI state per tick as CSV, plus a periodic summary.
/// The pose script stands in for the player: `far` parks out of the
/// awake radius, `start` parks at the authored level start, `hover`
/// glues to the herd centroid, `approach` flies in at carpet cruise
/// and then hovers, `orbit` circles the herd — the moving-wizard
/// cases the old fixture harness never exercised.
fn run_flock_probe(
    level: &LoadedLevel,
    out: &Path,
    ticks: u32,
    every: u32,
    pose_spec: &str,
    species: (u8, u8),
    strip: bool,
    dis: &[u16],
) -> Result<(), String> {
    use std::io::Write as _;

    let Some(init) = &level.world_init else {
        return Err(
            "--flock-probe needs the living world (do not pass --no-terrain-features)".to_string(),
        );
    };

    // Torus helpers (256-tile wrap), in TILE units.
    const N: f32 = 256.0;
    let wrap_d = |d: f32| (d + N / 2.0).rem_euclid(N) - N / 2.0;
    let dist = |a: (f32, f32), b: (f32, f32)| wrap_d(a.0 - b.0).hypot(wrap_d(a.1 - b.1));
    // Circular mean per axis — the herd centroid on the torus.
    let centroid = |pts: &[(f32, f32)]| -> Option<(f32, f32)> {
        if pts.is_empty() {
            return None;
        }
        let axis = |sel: fn(&(f32, f32)) -> f32| {
            let (mut s, mut c) = (0.0f32, 0.0f32);
            for p in pts {
                let a = sel(p) / N * std::f32::consts::TAU;
                s += a.sin();
                c += a.cos();
            }
            (s.atan2(c) / std::f32::consts::TAU * N).rem_euclid(N)
        };
        Some((axis(|p| p.0), axis(|p| p.1)))
    };
    // Connected components at LINK = 6 tiles; the pose scripts follow
    // the LARGEST cluster (level-000 authors several herds — the
    // global mean lands between them).
    let components = |pts: &[(f32, f32)]| -> Vec<Vec<usize>> {
        let n = pts.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            let mut c = x;
            while p[c] != r {
                let nx = p[c];
                p[c] = r;
                c = nx;
            }
            r
        }
        for a in 0..n {
            for b in (a + 1)..n {
                if dist(pts[a], pts[b]) <= 6.0 {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    parent[ra] = rb;
                }
            }
        }
        let mut groups: std::collections::HashMap<usize, Vec<usize>> = Default::default();
        for i in 0..n {
            let r = find(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
        let mut out: Vec<Vec<usize>> = groups.into_values().collect();
        out.sort_by_key(|g| std::cmp::Reverse(g.len()));
        out
    };

    // The minimal comparison environment: landscape + the tracked
    // species only (player request 2026-07-16) — no buildings, no
    // stage board, no rivals. The start marker survives so `start`
    // pose scripts stay meaningful.
    let world_init;
    let init = if strip {
        let i = WorldInit {
            game: init.game,
            planes: init.planes.clone(),
            things: init
                .things
                .iter()
                .filter(|t| {
                    (t.class == species.0 as u16 && t.model == species.1 as u16)
                        || (t.class == 10 && t.model == 0x52)
                        || (t.class == 3 && t.model == 4)
                })
                .cloned()
                .collect(),
            seed: init.seed,
            assets: init.assets.clone(),
            win_pct: 0,
            wizards: Default::default(),
            mc2_wizards: Default::default(),
            player_count: 1,
            stages: Vec::new(),
            stage_vars: Vec::new(),
            night_shade: init.night_shade,
            doom_level: init.doom_level,
            placeholders: init.placeholders,
            prune_owned_jars: false,
            chassis: init.chassis.clone(),
        };
        world_init = i;
        &world_init
    } else {
        init
    };
    // The level's raw StageVar rows — the herd-law bindings (graze
    // anchors, walk-to points, spawn gates) the tracked species may
    // attach to.
    for (s, v) in init.stage_vars.iter().enumerate() {
        if (v.0 as u8) & 0xF != 0 && v.0 as u8 != 0xFF {
            println!(
                "stagevar slot={s} index={:#04x} stage={} x={} y={} data={:#010x}",
                v.0 as u8, v.1, v.2, v.3, v.4
            );
        }
    }
    let mut w = init.build();
    for &d in dis {
        w.debug_fire_disposition(d);
        println!("fired disposition {d}");
    }

    // Pose script. Altitude args are TILES ABOVE THE HERD's mean
    // ground; the carpet cruises at the faithful 80 units/tick.
    const CRUISE: f32 = 80.0 / 256.0;
    let (mode, alt) = match pose_spec.split_once(':') {
        Some((m, a)) => (
            m,
            a.parse::<f32>()
                .map_err(|e| format!("--probe-pose {pose_spec}: bad altitude: {e}"))?,
        ),
        None => (pose_spec, 2.0),
    };
    let start = level.start.unwrap_or_default();
    let (mut px, mut py) = match mode {
        "far" => (2.0f32, 2.0f32),
        _ => (start.x, start.z),
    };
    let mut pz_alt = match mode {
        "far" => 40.0f32,
        _ => start.y,
    };
    let mut orbit_angle = 0.0f32;
    let mut approaching = matches!(mode, "approach" | "orbit");
    if !matches!(mode, "far" | "start" | "hover" | "approach" | "orbit") {
        return Err(format!(
            "--probe-pose {pose_spec}: unknown mode (far|start|hover[:ALT]|approach[:ALT]|orbit[:ALT])"
        ));
    }

    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut csv = std::io::BufWriter::new(file);
    writeln!(
        csv,
        "tick,slot,id,x,y,z,yaw,aim,speed,min_speed,max_speed,state,role,life,awake,leader,target,attacker,cadence,px,py,pdist,blocked"
    )
    .map_err(|e| e.to_string())?;

    // Cluster count (LINK = 6 tiles — the fixture harness's metric,
    // retail reads ~1-2).
    let clusters = |pts: &[(f32, f32)]| -> usize { components(pts).len() };

    // Attribution accumulators: goat-ticks by role x speed bucket.
    // Buckets: 0 = <=18 (walk), 1 = 19..=36 (catch-up), 2 = 37..=53,
    // 3 = >=54 (flee/min-speed).
    let bucket = |s: i16| -> usize {
        match s.abs() {
            0..=18 => 0,
            19..=36 => 1,
            37..=53 => 2,
            _ => 3,
        }
    };
    let mut attrib = [[0u64; 4]; 9]; // roles 0..7 + 8 = "other state"
    // Terrain-fence telemetry: goat-ticks with the move-core block
    // latch set (retail byte[2] & 4) vs total.
    let (mut blocked_ticks, mut total_ticks) = (0u64, 0u64);
    let n0 = w.debug_flock_probe(species.0, species.1).len();
    // The species' whole-map walkability (slope fence + tile-type
    // block), dumped once beside the CSV for the terrain-pocket
    // analysis: <out>.blockmap (raw 256x256 bytes, bit0 rough / bit1
    // type).
    if let Some(map) = w.debug_block_map(species.0, species.1) {
        let bm = out.with_extension("blockmap");
        std::fs::write(&bm, &map).map_err(|e| format!("{}: {e}", bm.display()))?;
        // The raw height plane beside it (the fence metric is height-
        // difference-driven; the terrain-provenance check reads this).
        let hp = out.with_extension("heights");
        std::fs::write(&hp, &w.planes().height).map_err(|e| format!("{}: {e}", hp.display()))?;
        let rough = map.iter().filter(|&&b| b & 1 != 0).count();
        let typ = map.iter().filter(|&&b| b & 2 != 0).count();
        println!(
            "block map: {} rough / {} type-blocked of 65536 tiles -> {}",
            rough,
            typ,
            bm.display()
        );
    }
    let idle = mgc_sim::mc1::world::PlayerCommand::default();
    println!(
        "flock probe: ({},{}) n={} pose={} ticks={} strip={} -> {}",
        species.0,
        species.1,
        n0,
        pose_spec,
        ticks,
        strip,
        out.display()
    );

    for t in 1..=ticks {
        // Advance the pose script from LAST tick's herd view.
        let rows = w.debug_flock_probe(species.0, species.1);
        let live: Vec<(f32, f32)> = rows
            .iter()
            .filter(|r| r.life >= 0)
            .map(|r| (r.x as f32 / 256.0, r.y as f32 / 256.0))
            .collect();
        // Follow the biggest herd, not the between-herds global mean.
        let c = components(&live)
            .first()
            .map(|g| g.iter().map(|&i| live[i]).collect::<Vec<_>>())
            .and_then(|pts| centroid(&pts));
        let ground = {
            let zs: Vec<f32> = rows
                .iter()
                .filter(|r| r.life >= 0)
                .map(|r| r.z as f32 / 256.0)
                .collect();
            if zs.is_empty() {
                pz_alt
            } else {
                zs.iter().sum::<f32>() / zs.len() as f32
            }
        };
        match (mode, c) {
            ("hover", Some(c)) => {
                px = c.0;
                py = c.1;
                pz_alt = ground + alt;
            }
            ("approach" | "orbit", Some(c)) => {
                if approaching {
                    let d = dist((px, py), c);
                    if d <= if mode == "orbit" { 4.0 } else { 1.0 } {
                        approaching = false;
                    } else {
                        let (dx, dy) = (wrap_d(c.0 - px), wrap_d(c.1 - py));
                        let step = CRUISE.min(d);
                        px = (px + dx / d * step).rem_euclid(N);
                        py = (py + dy / d * step).rem_euclid(N);
                        // Descend toward hover altitude on the way in.
                        pz_alt += ((ground + alt) - pz_alt).clamp(-0.1, 0.1);
                    }
                }
                if !approaching {
                    if mode == "orbit" {
                        orbit_angle += 0.02;
                        px = (c.0 + 4.0 * orbit_angle.cos()).rem_euclid(N);
                        py = (c.1 + 4.0 * orbit_angle.sin()).rem_euclid(N);
                    } else {
                        px = c.0;
                        py = c.1;
                    }
                    pz_alt = ground + alt;
                }
            }
            _ => {}
        }
        let pose = mgc_sim::mc1::world::PlayerPose::from_tiles(px, pz_alt, py, 0.0, 0.0, 0.0);
        w.tick(pose, idle);

        let rows = w.debug_flock_probe(species.0, species.1);
        for r in &rows {
            if r.life >= 0 {
                let role = r.state.wrapping_sub(8) as usize;
                attrib[if role < 8 { role } else { 8 }][bucket(r.speed)] += 1;
                total_ticks += 1;
                if r.flags & (1 << 27) != 0 {
                    blocked_ticks += 1;
                }
            }
            if t % every == 0 {
                let (gx, gy) = (r.x as f32 / 256.0, r.y as f32 / 256.0);
                let pd = dist((gx, gy), (px, py));
                writeln!(
                    csv,
                    "{t},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.1},{:.1},{pd:.2},{}",
                    r.slot,
                    r.id24,
                    r.x,
                    r.y,
                    r.z,
                    r.yaw,
                    r.aim,
                    r.speed,
                    r.min_speed,
                    r.max_speed,
                    r.state,
                    r.state.wrapping_sub(8),
                    r.life,
                    r.awake,
                    r.leader,
                    r.target,
                    r.attacker,
                    r.cadence,
                    px * 256.0,
                    py * 256.0,
                    (r.flags >> 27) & 1,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if t % 500 == 0 || t == ticks {
            let live: Vec<(f32, f32)> = rows
                .iter()
                .filter(|r| r.life >= 0)
                .map(|r| (r.x as f32 / 256.0, r.y as f32 / 256.0))
                .collect();
            let mut roles = [0usize; 9];
            let mut speeds = [0usize; 4];
            let mut fast = 0usize;
            for r in rows.iter().filter(|r| r.life >= 0) {
                let role = r.state.wrapping_sub(8) as usize;
                roles[if role < 8 { role } else { 8 }] += 1;
                speeds[bucket(r.speed)] += 1;
                if r.speed.abs() > 18 {
                    fast += 1;
                }
            }
            println!(
                "t={t}: alive={}/{n0} clusters={} roles[patrol={} wander={} chase={} FOLLOW={} flee={} other={}] speed[<=18:{} 19-36:{} 37-53:{} >=54:{}] fast={fast} player=({:.0},{:.0})",
                live.len(),
                clusters(&live),
                roles[0],
                roles[1],
                roles[2],
                roles[3],
                roles[6],
                roles[4] + roles[5] + roles[7] + roles[8],
                speeds[0],
                speeds[1],
                speeds[2],
                speeds[3],
                px,
                py
            );
        }
    }

    println!(
        "\nterrain fence: {blocked_ticks}/{total_ticks} goat-ticks with the block latch set ({:.2}%)",
        100.0 * blocked_ticks as f64 / total_ticks.max(1) as f64
    );
    println!("attribution (goat-ticks by role x speed bucket):");
    println!("  role         <=18     19-36    37-53    >=54");
    let names = [
        "patrol", "wander", "chase", "FOLLOW", "prekill", "kill", "flee", "role7", "other",
    ];
    for (i, name) in names.iter().enumerate() {
        let row = &attrib[i];
        if row.iter().any(|&v| v != 0) {
            println!(
                "  {name:<10} {:>8} {:>8} {:>8} {:>8}",
                row[0], row[1], row[2], row[3]
            );
        }
    }
    csv.flush().map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}

/// The MC2 environment's sky/fog color, sRGB: the mode of the bundle
/// shade LUT's row 0 — the engine's fog FAR color, i.e. what distant
/// terrain fades into (night/cave = black, day = pale blue; a few
/// row-0 entries deviate for reserved/animated palette slots, hence
/// the mode). None for MC1/HW — their certified presentation keeps
/// the renderer's hand-picked haze constant until the sky trace
/// lands (the same TABLES row-0 structure exists there too).
fn mc2_sky_srgb(level: &LoadedLevel) -> Option<[f32; 3]> {
    if !matches!(level.game, mgc_sim::ids::GameId::Mc2) {
        return None;
    }
    let row0 = level.view.shade_lut.get(..256)?;
    let mut counts = [0u16; 256];
    for &i in row0 {
        counts[i as usize] += 1;
    }
    let idx = (0..256).max_by_key(|&i| counts[i])?;
    let rgb = level.view.palette[idx];
    Some([
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    ])
}

/// Apply the playtest instruments to a freshly built world — ONE place
/// so a future instrument can't miss a call site (fresh start in
/// `App::new`, `restart_level`, and the headless screenshot path all
/// go through here).
fn apply_instruments(
    w: &mut mgc_sim::mc1::world::World,
    dev_spells: bool,
    plausible_spells: &[u8],
    plausible_book_mc2: &[(u8, i32)],
    invincible: bool,
) {
    if dev_spells {
        w.set_dev_spells(true);
    }
    if !plausible_spells.is_empty() {
        w.grant_spells(plausible_spells);
    }
    if !plausible_book_mc2.is_empty() {
        w.mc2_grant_plausible(plausible_book_mc2);
    }
    if invincible {
        w.set_invincible(true);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_screenshot(
    mut level: LoadedLevel,
    out: &Path,
    camera: Option<[f32; 5]>,
    smooth_shading: bool,
    map_view: bool,
    anim_turn: f32,
    map_triggers: bool,
    dev_spells: bool,
    cfg_hud_transparent: bool,
) -> Result<(), String> {
    // Same 2×-native 4:3 size as the live default window: integer
    // pixel grid (no fractional-scale aliasing), retail aspect.
    let mut renderer = Renderer::offscreen(1280, 960).map_err(|e| e.to_string())?;
    let overlay = mgc_render::MapOverlay {
        dots: level.map_dots.clone(),
        areas: if map_triggers {
            level.map_areas.clone()
        } else {
            Vec::new()
        },
    };
    renderer.load_level(&level.view, &overlay);
    renderer.set_map_stamps(level.map_stamps.clone());
    // Objective-guide marks in map-view captures (steady, no blink).
    if let Some(w) = &level.world {
        let marks: Vec<_> = w
            .mc2_objective_targets()
            .into_iter()
            .map(|t| mgc_render::ObjectiveMark {
                x: t.x,
                z: t.z,
                nearest: t.nearest,
                yellow: t.yellow,
            })
            .collect();
        // Tick 68 = both blink gates "on" (outline 1-in-4 + arrow window),
        // so a still capture shows the full overlay.
        renderer.set_objective_marks(marks, 68);
    }
    if let Some((index, atlas)) = &level.sprites {
        renderer.load_sprites(index.clone(), atlas);
    }
    if let Some(assets) = &level.ui {
        renderer.load_ui_atlas(assets.atlas_w, assets.atlas_h, &assets.atlas_rgba);
        if let Ok(p) = std::env::var("MGC_DUMP_UI_ATLAS") {
            write_png(
                Path::new(&p),
                assets.atlas_w,
                assets.atlas_h,
                &assets.atlas_rgba,
            )?;
        }
    }
    renderer.set_billboards(level.billboards.clone());
    renderer.set_smooth_shading(smooth_shading);
    if let Some(sky) = mc2_sky_srgb(&level) {
        renderer.set_sky_color(sky);
    }
    // HUD transparency: the config decides (same path as live play);
    // MGC_HUD_OPAQUE overrides for A/B captures — by VALUE, so
    // MGC_HUD_OPAQUE=0 forces transparent and =1 forces opaque; an
    // unrecognized value warns and defers to the config.
    let hud_transparent = match std::env::var("MGC_HUD_OPAQUE") {
        Ok(v) => match v.as_str() {
            "" | "0" | "false" | "off" => true,
            "1" | "true" | "on" => false,
            other => {
                eprintln!("MGC_HUD_OPAQUE={other} not understood (use 0/1); using config");
                cfg_hud_transparent
            }
        },
        Err(_) => cfg_hud_transparent,
    };
    renderer.set_hud_transparent(hud_transparent);
    renderer.set_map_view(map_view);
    // Screenshots follow the game's faithful map topology (no config
    // override in the headless path): MC2 = the split layout.
    let shot_is_mc2 = matches!(level.game, mgc_sim::ids::GameId::Mc2);
    renderer.set_map_layout(if shot_is_mc2 {
        mgc_render::MapScreenLayout::Mc2Split
    } else {
        mgc_render::MapScreenLayout::Mc1Book
    });
    renderer.set_anim_turn(anim_turn);
    // Spell UI (book grid or HUD), from the level-start loadout.
    if let (Some(assets), Some(w)) = (&level.ui, &mut level.world) {
        // invincible=false: a single headless frame takes no damage.
        apply_instruments(
            w,
            dev_spells,
            &level.plausible_spells,
            &level.plausible_book_mc2,
            false,
        );
        let loadout = w.loadout();
        let vitals = w.vitals();
        let mc2_book = shot_is_mc2.then(|| w.mc2_book_view());
        let quads = if map_view {
            if shot_is_mc2 {
                // MC2's map screen has no book half; the split layout
                // shows the stretched live view instead.
                Vec::new()
            } else {
                ui::book_quads(assets, &loadout, &[None; 10], 1280.0, 960.0, (-1.0, -1.0)).0
            }
        } else {
            // alert_blink=true: a screenshot shows any armed alert.
            ui::hud_quads(
                assets,
                &loadout,
                &vitals,
                hud_transparent,
                true,
                shot_is_mc2,
                mc2_book.as_ref(),
                1280.0,
                960.0,
            )
        };
        renderer.set_ui_quads(quads);
    }
    let flyer = level.start.unwrap_or_default();
    let [x, y, z, yaw_deg, pitch_deg] = camera.unwrap_or([
        flyer.x,
        flyer.y,
        flyer.z,
        flyer.yaw.to_degrees(),
        flyer.pitch.to_degrees(),
    ]);
    let cam = CameraView {
        x,
        y,
        z,
        yaw: yaw_deg.to_radians(),
        pitch: pitch_deg.to_radians(),
        roll: 0.0,
        fov_y: FOV_Y,
    };
    renderer.render(&cam).map_err(|e| format!("render: {e}"))?;
    let (w, h, rgba) = renderer.read_offscreen();
    write_png(out, w, h, &rgba)?;
    println!("{} -> {} ({}x{})", level.label, out.display(), w, h);
    Ok(())
}

fn main() -> std::process::ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return std::process::ExitCode::from(2);
        }
    };
    let (config_path, explicit) = match &args.config {
        Some(p) => (p.clone(), true),
        None => (PathBuf::from(config::DEFAULT_PATH), false),
    };
    let mut cfg = match config::Config::load(&config_path, explicit) {
        Ok(c) => {
            if config_path.exists() {
                println!("config: {}", config_path.display());
            }
            c
        }
        Err(e) => {
            eprintln!("error: config: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // Fold the one-run CLI overrides into the resolved config, so `cfg`
    // is the single source of truth from here on (the App reads it live;
    // the startup summary and a future menu are views over it).
    let en = &mut cfg.render.enhancement;
    if let Some(v) = args.smooth_shading {
        en.smooth_shading = v;
    }
    if let Some(v) = args.expose_jar_spells {
        en.expose_jar_spells = v;
    }
    let de = &mut cfg.render.debug;
    if let Some(v) = args.map_triggers {
        de.map_trigger_areas = v;
    }
    if let Some(v) = args.health_bars {
        de.health_bars = v;
    }
    if let Some(v) = args.crosshair {
        de.crosshair = v;
    }
    if let Some(v) = args.grace_meter {
        de.grace_meter = v;
    }
    if let Some(v) = args.thrust {
        cfg.controls.models.thrust = v;
    }
    if let Some(v) = args.altitude {
        cfg.controls.models.altitude = v;
    }
    if let Some(v) = args.bindings {
        cfg.controls.preferences.bindings = v;
    }
    if let Some(v) = args.spell_selector {
        cfg.gameplay.enhancement.spell_selector = v;
    }
    if let Some(v) = args.prune_owned_jars {
        cfg.gameplay.enhancement.prune_owned_jars = v;
    }
    if let Some(v) = args.dev_spells {
        cfg.gameplay.cheat.dev_spells = v;
    }
    if let Some(v) = args.invincible {
        cfg.gameplay.cheat.invincible = v;
    }
    if let Some(v) = args.plausible_spellbook {
        cfg.dev.plausible_spellbook = v;
    }
    // The entity pool is an OFFLINE parameter: CLI wins over config, and
    // the effective value is reflected back for the summary. The config
    // path applies the CLI's 2..=60000 guard too — slot indices are u16,
    // and an unvalidated 70000 would silently truncate the free stack.
    let pool_slots = args
        .pool_slots
        .or(cfg.sim.parameters.entity_pool_size.map(|n| n as usize));
    if let Some(n) = pool_slots
        && !(2..=60000).contains(&n)
    {
        eprintln!("error: sim.parameters.entity_pool_size must be in 2..=60000 (slots are u16)");
        return std::process::ExitCode::FAILURE;
    }
    cfg.sim.parameters.entity_pool_size = pool_slots.map(|n| n as u32);
    // Same offline pattern for the wake radius: CLI wins over config,
    // effective value reflected back for the summary.
    let awake_range = args.awake_range.or(cfg.sim.parameters.awake_range);
    cfg.sim.parameters.awake_range = awake_range;

    // First-run / stale-epoch auto-bake: regenerate the baked tree
    // from the original game data before touching it.
    if let Err(e) = bakecheck::ensure_baked(&args.level, cfg.gamedata.as_deref()) {
        eprintln!("error: {e}");
        return std::process::ExitCode::FAILURE;
    }

    let level = match load_level(
        &args.level,
        args.tileset,
        args.terrain_features,
        cfg.dev.plausible_spellbook,
        cfg.gameplay.enhancement.prune_owned_jars,
        pool_slots,
        awake_range,
    ) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Some(out) = &args.map {
        return match run_map(
            &level,
            out,
            args.map_scale,
            cfg.render.debug.map_trigger_areas,
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    if let Some(out) = &args.flock_probe {
        return match run_flock_probe(
            &level,
            out,
            args.probe_ticks,
            args.probe_every,
            &args.probe_pose,
            args.probe_species,
            args.probe_strip,
            &args.probe_dis,
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    if let Some(out) = &args.screenshot {
        return match run_screenshot(
            level,
            out,
            args.camera,
            cfg.render.enhancement.smooth_shading,
            args.map_view,
            args.anim_turn,
            cfg.render.debug.map_trigger_areas,
            cfg.gameplay.cheat.dev_spells,
            matches!(
                cfg.render.enhancement.hud_transparency,
                config::HudTransparency::Mc1
            ),
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    // Spell-selector surfaces, resolved against the loaded game. MC2
    // owns exactly one shape (the CTRL pane) — an explicit map-book
    // request there coerces with a note rather than inventing a
    // 26-spell in-map grid.
    let selector_choice = cfg.gameplay.enhancement.spell_selector;
    let level_is_mc2 = matches!(level.game, mgc_sim::ids::GameId::Mc2);
    let selector = selector_choice.resolve(level_is_mc2);
    if level_is_mc2
        && matches!(
            selector_choice,
            config::SpellSelector::Mc1 | config::SpellSelector::Mc1Mc2
        )
    {
        println!("spell-selector: MC2 has no in-map spellbook — using the faithful CTRL pane");
    }

    println!("mgcarpet {}", env!("CARGO_PKG_VERSION"));
    let move_keys = match cfg.controls.preferences.bindings {
        config::Bindings::Classic => "Up/Down arrows accel/decel, Left/Right strafe",
        config::Bindings::Wasd => "W/S accel/decel, A/D strafe",
    };
    match cfg.controls.models.thrust {
        config::ThrustModel::Mc1 => println!(
            "controls: faithful MC1 — mouse = stick (offset steers, recenter to fly straight),\n\
             \x20         {move_keys} (impulses: speed persists until countered),"
        ),
        config::ThrustModel::Enhanced => {
            println!("controls: enhanced — mouse look, {move_keys} (hold-to-fly),")
        }
    }
    if cfg.controls.models.altitude == config::AltitudeModel::ExtendedLift {
        println!("          E/Q float up/down (extended lift, capped at the highest terrain),");
    }
    println!("          Backspace full-stops the carpet (speed + steering; MC2's stabilize key),");
    println!("          Space respawns after death (at your castle; no castle = level restart),");
    println!("          Shift+L demolishes your own castle one level per press,");
    println!("          LMB/RMB cast the equipped hand's spell (hold = channel),");
    if selector.map_book {
        println!("          Enter opens the book: click a spell with LMB/RMB to equip,");
    } else {
        println!("          Enter opens the map screen,");
    }
    if selector.ctrl_pane {
        println!("          hold Ctrl for the spell selector: click LMB/RMB to equip a hand,");
    }
    println!("          hover + 1-9,0 binds a quick key (in flight: equip, Shift = right hand),");
    println!("          Esc twice quits.");

    // The structured options summary: every toggle, its current value,
    // the alternatives (faithful `*`-marked), and how to change it.
    settings::print_summary(&cfg, level.game, &level.label);

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("error: cannot create event loop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App::new(level, cfg);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: event loop: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
