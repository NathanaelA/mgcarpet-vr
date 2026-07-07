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

mod config;
mod entities;
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

/// Pristine inputs to rebuild the [`mgc_sim::world::World`] for a
/// LEVEL RESTART — the original's castle-less-death "lost + level
/// over" flow ends in exactly this (respawn at the start of a fresh
/// level).
struct WorldInit {
    planes: mgc_sim::features::Planes,
    things: Vec<mgc_formats::Thing>,
    seed: u32,
    assets: mgc_sim::features::FeatureAssets,
    win_pct: u16,
}

impl WorldInit {
    fn build(&self) -> mgc_sim::world::World {
        let mut w = mgc_sim::world::World::new(
            self.planes.clone(),
            &self.things,
            self.seed,
            self.assets.clone(),
        );
        if self.win_pct > 0 {
            w.set_win_pct(self.win_pct);
        }
        w
    }
}

struct LoadedLevel {
    view: LevelView,
    height: Vec<u8>,
    label: String,
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
    world: Option<mgc_sim::world::World>,
    /// Rebuild inputs for the castle-less-death level restart.
    world_init: Option<WorldInit>,
    /// Bundle palette, kept for runtime map-dot rebuilds.
    palette_rgba: [[u8; 4]; 256],
    /// The per-game audio bundle directory (`assets/mc1-audio` /
    /// `mc2-audio`), when baked.
    audio_dir: Option<PathBuf>,
    /// INTERIM level-music pick (MC1 cgame1-3 by level index; MC2
    /// redbook by index) until the original's per-level song command
    /// (level struct +576 / the script cases 12/25) is decoded.
    music_track: Option<String>,
    /// HSPR UI sprites composited to RGBA (spellbook/HUD); None when
    /// the bundle has no UI members (MC2 until its UI track).
    ui: Option<ui::UiAssets>,
    /// Live trigger/portal volumes for the opt-in map overlay.
    map_areas: Vec<mgc_render::MapArea>,
    /// Castle/balloon icon patches for the map marker pass.
    map_icons: entities::MapIcons,
    /// Live icon stamps (own castle/balloons), refreshed per tick.
    map_stamps: Vec<mgc_render::MapStamp>,
}

/// Resolve the world's live volumes into map overlay circles: amber =
/// fly-into triggers, red = kill-watchers, cyan = collected-item
/// triggers, violet = portals.
fn map_areas(world: &mgc_sim::world::World) -> Vec<mgc_render::MapArea> {
    use mgc_sim::world::VolumeKind;
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
                VolumeKind::Inventory => [64, 208, 255],
                VolumeKind::Portal => [208, 96, 255],
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
/// — mgc_sim::features) to the pristine baked terrain, as the engine
/// does. Off = the raw generator output, for comparison renders.
fn load_level(
    level_path: &Path,
    tileset: Option<u8>,
    terrain_features: bool,
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

    // The living world (MC1/HW; MC2's feature/entity semantics are a
    // separate remc2 port, pending): the load-time feature pass, then
    // disposition 0 spawns the initial population — things authored
    // behind triggers (dis_id != 0) stay latent until fired. Needs the
    // shading + angle planes and the bundle's search/build data.
    let mut world = None;
    let mut world_init = None;
    if terrain_features && package.meta.game != Game::MagicCarpet2 {
        match (
            &shading,
            &angle,
            &bundle.search,
            &bundle.build_tab,
            &bundle.build_dat,
        ) {
            (Some(sh), Some(an), Some(search), Some(build_tab), Some(build_dat)) => {
                let assets = mgc_sim::features::FeatureAssets::parse(search, build_tab, build_dat)?;
                let seed = package.gen_params.as_ref().map_or(0, |g| g.seed);
                // The level goal: footer[0] = the required banked
                // percentage of world mana (level offset 38800 —
                // the win check's threshold and the HUD goal tick).
                let win_pct = package
                    .gen_params
                    .as_ref()
                    .and_then(|g| g.footer)
                    .map_or(0, |f| f[0]);
                let init = WorldInit {
                    planes: mgc_sim::features::Planes {
                        height: height.clone(),
                        tile_type: tile_type.clone(),
                        shading: sh.clone(),
                        angle: an.clone(),
                    },
                    things: package.things.things.clone(),
                    seed,
                    assets,
                    win_pct,
                };
                let w = init.build();
                // The view starts from the post-feature planes.
                height.copy_from_slice(&w.planes().height);
                tile_type.copy_from_slice(&w.planes().tile_type);
                shading.as_mut().unwrap().copy_from_slice(&w.planes().shading);
                angle.as_mut().unwrap().copy_from_slice(&w.planes().angle);
                world = Some(w);
                world_init = Some(init);
            }
            (None, ..) | (_, None, ..) => eprintln!(
                "note: package lacks shading/angle planes — terrain features skipped (rebake)"
            ),
            _ => eprintln!(
                "note: bundle lacks search/build data — terrain features skipped (rebake)"
            ),
        }
    }

    // World entities as billboards + map dots. With a live world, the
    // sim's pose snapshot is the source of truth (sprite types, spawn
    // facing and jitter come from the ported spawn handlers); without
    // one (MC2, --no-terrain-features), every drawable record resolves
    // statically — the old behavior, kept as the comparison mode.
    let (billboards, map_dots) = if package.meta.game != Game::MagicCarpet2 {
        let index = bundle.sprites.as_ref().map(|(i, _)| i);
        let dims = |id: u16| {
            index
                .and_then(|i| i.sprites.get(id as usize))
                .map(|s| (s.width, s.height))
        };
        match &world {
            Some(w) => {
                let poses = w.live_poses();
                (
                    entities::billboards_from_poses(&poses, dims),
                    // No dwelling is claimed at load time, so the
                    // owned-buildings highlight is vacuously off here
                    // (and the blink phase starts low).
                    entities::map_dots_from_poses(&poses, &bundle.palette, false, false),
                )
            }
            None => (
                entities::billboards(&package.things.things, &height, dims),
                entities::map_dots(&package.things.things, &bundle.palette),
            ),
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // The original's spawn: the class-3 m4 start marker's position,
    // hovering over the (post-feature) terrain, facing north.
    let start = entities::player_start(&package.things.things).map(|(x, z)| Flyer {
        x,
        y: entities::ground_at(&height, x, z) + entities::START_HOVER,
        z,
        yaw: 0.0,
        pitch: 0.0,
        ..Flyer::default()
    });

    let ui_assets = bundle
        .ui_sprites
        .as_ref()
        .map(|(idx, px)| {
            ui::UiAssets::build(idx.clone(), px, &bundle.palette, bundle.blend_lut.as_deref())
        });

    // Per-game audio bundle + the INTERIM music pick.
    let audio_game = if package.meta.game == Game::MagicCarpet2 {
        "mc2"
    } else {
        "mc1"
    };
    let audio_dir = {
        let d = baked_root.join("assets").join(format!("{audio_game}-audio"));
        d.is_dir().then_some(d)
    };
    let music_track = Some(if audio_game == "mc2" {
        format!("track-{:02}", 2 + package.meta.level as usize % 27)
    } else {
        format!("cgame{}", 1 + package.meta.level as usize % 3)
    });

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
        },
        height,
        label: format!("{game} level {}", package.meta.level),
        sprites: bundle.sprites,
        billboards,
        map_dots,
        start,
        map_areas: world.as_ref().map(map_areas).unwrap_or_default(),
        world,
        world_init,
        palette_rgba: bundle.palette,
        map_icons: entities::MapIcons {
            // Castle = UI sprite 58+team, balloon = 66+team (team 0);
            // remc1 sub_48710 :57230/:57234.
            castle: ui_assets.as_ref().and_then(|u| u.map_stamp(58)),
            balloon: ui_assets.as_ref().and_then(|u| u.map_stamp(66)),
        },
        map_stamps: Vec::new(),
        ui: ui_assets,
        audio_dir,
        music_track,
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
    smooth_shading: bool,
    /// Map trigger-volume overlay (enhancement/debug; V toggles).
    map_triggers: bool,
    /// Monster health bars (enhancement/debug; H toggles).
    health_bars: bool,
    /// All spells + infinite mana (playtest instrument; G toggles).
    dev_spells: bool,
    /// The pre-mortality invincible player (config `invincible`).
    invincible: bool,
    /// Space pressed since the last sim tick (respawn confirm).
    pending_respawn: bool,
    /// Shift+L pressed since the last sim tick (castle demolish).
    pending_demolish: bool,
    /// Claimed dwellings highlighted on the map (MC2-style opt-in).
    map_owned_buildings: bool,
    /// Own castle position in tile units (the guide-path target),
    /// refreshed from the pose set.
    castle_pos: Option<(f32, f32)>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    sim: Simulation,
    prev_flyer: Flyer,
    keys: HeldKeys,
    mouse: MouseAccum,
    /// Flight-control tiers (thrust/altitude models mirror into the
    /// sim; bindings + sensitivity are app-side input mapping).
    flight: config::FlightConfig,
    stick: VirtualStick,
    /// Left/right button held while grabbed: the two casting hands.
    fire_held: bool,
    fire_right_held: bool,
    grabbed: bool,
    /// Cursor position in window pixels (book-screen interactions).
    cursor: (f32, f32),
    /// Spell under the cursor on the book screen (display hit test,
    /// refreshed each frame the book is open).
    hovered: Option<mgc_sim::spells::SpellId>,
    /// Quick-key bindings 1..9,0 → spell id (session-local; set in the
    /// book by hovering + pressing a digit). Our enhancement — the
    /// original only has the obscure Ctrl+]+digit chord.
    quick_binds: [Option<u8>; 10],
    /// Equip requests to feed the next sim tick (LMB hand, RMB hand).
    pending_equip: (Option<u8>, Option<u8>),
    shift_held: bool,
    last_frame: std::time::Instant,
    accumulator: f32,
    /// Audio runtime (None in headless paths / when opening failed).
    audio: Option<mgc_audio::Audio>,
    /// F1/F2 runtime toggles (the original's keys) over the config's
    /// audio preferences.
    sound_on: bool,
    music_on: bool,
    sfx_volume: f32,
    music_volume: f32,
}

impl App {
    fn new(
        mut level: LoadedLevel,
        smooth_shading: bool,
        map_triggers: bool,
        health_bars: bool,
        dev_spells: bool,
        invincible: bool,
        map_owned_buildings: bool,
        audio_cfg: &config::AudioConfig,
        flight: config::FlightConfig,
    ) -> Self {
        // Audio: open the device, load the game's audio bundle, start
        // the level music. Any failure degrades to silence, never to
        // an unplayable game.
        let mut audio = None;
        if audio_cfg.sound || audio_cfg.music {
            let mut a = mgc_audio::Audio::open();
            if let Some(dir) = &level.audio_dir {
                if let Err(e) = a.load_bundle(dir, 0) {
                    eprintln!("note: audio bundle: {e}");
                }
            } else {
                eprintln!("note: no audio bundle baked — sound effects disabled (rebake)");
            }
            a.set_volumes(
                if audio_cfg.sound { audio_cfg.sfx_volume } else { 0.0 },
                if audio_cfg.music { audio_cfg.music_volume } else { 0.0 },
            );
            if audio_cfg.music {
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
        sim.thrust_model = match flight.thrust {
            config::ThrustModel::Mc1 => mgc_sim::ThrustModel::Mc1,
            config::ThrustModel::Enhanced => mgc_sim::ThrustModel::Enhanced,
        };
        sim.altitude_model = match flight.altitude {
            config::AltitudeModel::Faithful => mgc_sim::AltitudeModel::Faithful,
            config::AltitudeModel::ExtendedLift => mgc_sim::AltitudeModel::ExtendedLift,
        };
        if let Some(start) = level.start {
            sim.flyer = start;
            sim.sync_carpet_from_flyer();
        }
        if dev_spells {
            if let Some(w) = &mut sim.world {
                w.set_dev_spells(true);
            }
        }
        if invincible {
            if let Some(w) = &mut sim.world {
                w.set_invincible(true);
            }
        }
        let prev_flyer = sim.flyer;
        Self {
            level,
            smooth_shading,
            map_triggers,
            health_bars,
            dev_spells,
            invincible,
            pending_respawn: false,
            pending_demolish: false,
            map_owned_buildings,
            castle_pos: None,
            window: None,
            renderer: None,
            sim,
            prev_flyer,
            keys: HeldKeys::default(),
            mouse: MouseAccum::default(),
            flight,
            stick: VirtualStick::default(),
            fire_held: false,
            fire_right_held: false,
            grabbed: false,
            cursor: (0.0, 0.0),
            hovered: None,
            quick_binds: [None; 10],
            pending_equip: (None, None),
            shift_held: false,
            last_frame: std::time::Instant::now(),
            accumulator: 0.0,
            audio,
            sound_on: audio_cfg.sound,
            music_on: audio_cfg.music,
            sfx_volume: audio_cfg.sfx_volume,
            music_volume: audio_cfg.music_volume,
        }
    }

    /// Per-sim-tick audio: drain the world's sound requests into the
    /// faithful mixer, feed the ambient rule, run the flush.
    fn audio_tick(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        let f = &self.sim.flyer;
        let pose = mgc_sim::world::PlayerPose::from_tiles(f.x, f.y, f.z, f.yaw, f.pitch, 0.0);
        let listener = mgc_audio::Listener {
            pos: (pose.x, pose.y, pose.z),
            yaw: pose.heading,
        };
        if let Some(w) = &mut self.sim.world {
            let frame = w.take_audio(pose);
            if self.sound_on {
                for e in frame.events {
                    let source = if e.player {
                        mgc_audio::Source::Player
                    } else {
                        mgc_audio::Source::World {
                            pos: e.pos,
                            tag: e.tag,
                        }
                    };
                    audio.event(e.id, source, &listener);
                }
                audio
                    .mixer
                    .set_ambient(frame.over_water, frame.fire_near, frame.market_near);
            }
            audio.set_danger(frame.danger);
        }
        audio.tick();
    }

    /// Castle-less death: rebuild the pristine world (the original
    /// restarts the level) and reset the flyer to the level start.
    fn restart_level(&mut self) {
        let Some(init) = &self.level.world_init else {
            return;
        };
        let mut w = init.build();
        if self.dev_spells {
            w.set_dev_spells(true);
        }
        if self.invincible {
            w.set_invincible(true);
        }
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
        let mc1 = self.flight.thrust == config::ThrustModel::Mc1;
        // Explicit float up/down is the extended-lift enhancement; the
        // faithful altitude model has no vertical control at all.
        let lift_keys = self.flight.altitude == config::AltitudeModel::ExtendedLift;
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
            equip_left: self.pending_equip.0.take().map(mgc_sim::spells::SpellId),
            equip_right: self.pending_equip.1.take().map(mgc_sim::spells::SpellId),
            respawn: std::mem::take(&mut self.pending_respawn),
            demolish: std::mem::take(&mut self.pending_demolish),
            ..Default::default()
        };
        if mc1 {
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
        let terrain = w.terrain_dirty;
        let entities = w.entities_dirty;
        if terrain {
            let (Some(shading), Some(angle)) =
                (self.level.view.shading.as_mut(), self.level.view.angle.as_mut())
            else {
                return;
            };
            w.copy_planes_into(mgc_sim::features::TerrainPlanes {
                height: &mut self.level.view.height,
                tile_type: &mut self.level.view.tile_type,
                shading,
                angle,
            });
        }
        let mut bars = Vec::new();
        if entities {
            let poses = w.live_poses();
            let index = self.level.sprites.as_ref().map(|(i, _)| i);
            let dims = |id: u16| {
                index
                    .and_then(|i| i.sprites.get(id as usize))
                    .map(|s| (s.width, s.height))
            };
            self.level.billboards = entities::billboards_from_poses(&poses, dims);
            if self.health_bars {
                bars = entities::health_bars_from_poses(&poses, dims);
            }
            self.level.map_dots = entities::map_dots_from_poses(
                &poses,
                &self.level.palette_rgba,
                self.map_owned_buildings,
                // The claimed-ball blink phase (the original's global
                // toggle byte; ~4 Hz at 30 ticks/s reads right).
                self.sim.tick >> 3 & 1 == 0,
            );
            self.level.map_stamps = entities::map_stamps_from_poses(&poses, &self.level.map_icons);
            self.castle_pos = poses
                .iter()
                .find(|p| p.class == 3 && p.model == 2 && p.player_owned)
                .map(|p| (p.x, p.z));
            self.level.map_areas = map_areas(w);
        }
        w.terrain_dirty = false;
        w.entities_dirty = false;
        let overlay = self.map_overlay();
        if let Some(r) = &mut self.renderer {
            if entities {
                r.set_billboards(self.level.billboards.clone());
                r.set_health_bars(bars);
            }
            if terrain {
                r.update_terrain(&self.level.view, &overlay);
            } else {
                // The map recomposes EVERY frame — the original
                // redraws it per frame, and the blink + marching-ants
                // phases live in the pixels (the old every-8th-tick
                // throttle was the player-reported low refresh rate).
                r.update_map(&self.level.view, &overlay);
            }
        }
    }

    /// Assemble the current map overlay: dots + own-castle/balloon
    /// icon stamps + the guide path (player → own castle, marching
    /// ants on the tick phase) + the opt-in trigger circles.
    fn map_overlay(&self) -> mgc_render::MapOverlay {
        mgc_render::MapOverlay {
            dots: self.level.map_dots.clone(),
            areas: if self.map_triggers {
                self.level.map_areas.clone()
            } else {
                Vec::new()
            },
            stamps: self.level.map_stamps.clone(),
            path: self.castle_pos.map(|(cx, cz)| mgc_render::MapPath {
                from: (self.sim.flyer.x, self.sim.flyer.z),
                to: (cx, cz),
                phase: (self.sim.tick & 3) as u8,
            }),
        }
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
        let attrs =
            Window::default_attributes().with_title(format!("Magic Carpet — {}", self.level.label));
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
                renderer.set_smooth_shading(self.smooth_shading);
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
                if self.book_open() {
                    // Book screen: clicking an owned spell binds it to
                    // that hand (the original's commands 0x15/0x16)
                    // AND closes the book back into flight (player-
                    // confirmed original UX). Clicks on unowned slots
                    // or empty page do nothing.
                    if down {
                        let owned = self
                            .sim
                            .world
                            .as_ref()
                            .map(|w| w.loadout().owned)
                            .unwrap_or([false; 24]);
                        if let Some(spell) = self.hovered {
                            if owned[spell.0 as usize] {
                                match button {
                                    MouseButton::Left => {
                                        self.pending_equip.0 = Some(spell.0)
                                    }
                                    MouseButton::Right => {
                                        self.pending_equip.1 = Some(spell.0)
                                    }
                                    _ => return,
                                }
                                if let Some(r) = &mut self.renderer {
                                    r.set_map_view(false);
                                }
                                self.set_grab(true);
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
                                    self.quick_binds[d] = Some(spell.0);
                                    println!(
                                        "quick key {}: {}",
                                        (d + 1) % 10,
                                        spell.name()
                                    );
                                }
                            } else if let Some(spell) = self.quick_binds[d] {
                                if self.shift_held {
                                    self.pending_equip.1 = Some(spell);
                                } else {
                                    self.pending_equip.0 = Some(spell);
                                }
                            }
                            return;
                        }
                    }
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::F1) {
                    // The original's sound toggle (remc1 :20086).
                    self.sound_on = !self.sound_on;
                    if let Some(a) = &mut self.audio {
                        a.set_volumes(
                            if self.sound_on { self.sfx_volume } else { 0.0 },
                            if self.music_on { self.music_volume } else { 0.0 },
                        );
                    }
                    println!("sound: {}", if self.sound_on { "on" } else { "off" });
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::F2) {
                    // The original's music toggle (remc1 :20100).
                    self.music_on = !self.music_on;
                    if let Some(a) = &mut self.audio {
                        if self.music_on {
                            if let Some(track) = &self.level.music_track {
                                let _ = a.play_music(track, true);
                            }
                            a.set_volumes(
                                if self.sound_on { self.sfx_volume } else { 0.0 },
                                self.music_volume,
                            );
                        } else {
                            a.stop_music();
                        }
                    }
                    println!("music: {}", if self.music_on { "on" } else { "off" });
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyT) {
                    self.smooth_shading = !self.smooth_shading;
                    if let Some(r) = &mut self.renderer {
                        r.set_smooth_shading(self.smooth_shading);
                    }
                    println!(
                        "shading: {}",
                        if self.smooth_shading {
                            "smooth (enhanced)"
                        } else {
                            "per-tile (original)"
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyV) {
                    self.map_triggers = !self.map_triggers;
                    let overlay = self.map_overlay();
                    if let Some(r) = &mut self.renderer {
                        r.update_map(&self.level.view, &overlay);
                    }
                    println!(
                        "map trigger overlay: {}",
                        if self.map_triggers {
                            "on (enhanced)"
                        } else {
                            "off (original)"
                        }
                    );
                    return;
                }
                // The demolish key (MC1 Shift+L, scancode 0x26 under
                // the shift branch :20496-501): razes the OWN castle
                // one level per press — the castle-as-attack-spell
                // enabler, at the price of the respawn point.
                if down
                    && self.shift_held
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyL)
                {
                    self.pending_demolish = true;
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyG) {
                    self.dev_spells = !self.dev_spells;
                    if let Some(w) = &mut self.sim.world {
                        w.set_dev_spells(self.dev_spells);
                    }
                    println!(
                        "dev spells: {}",
                        if self.dev_spells {
                            "on — all spells, infinite mana (playtest instrument)"
                        } else {
                            "off (authentic acquisition/mana)"
                        }
                    );
                    return;
                }
                if down && event.physical_key == PhysicalKey::Code(KeyCode::KeyH) {
                    self.health_bars = !self.health_bars;
                    if !self.health_bars {
                        if let Some(r) = &mut self.renderer {
                            r.set_health_bars(Vec::new());
                        }
                    }
                    // On: bars appear with the next entity sync (every
                    // tick while creatures move).
                    println!(
                        "monster health bars: {}",
                        if self.health_bars {
                            "on (debug enhancement)"
                        } else {
                            "off (original)"
                        }
                    );
                    return;
                }
                let wasd = self.flight.bindings == config::Bindings::Wasd;
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

                while self.accumulator >= TICK_DT {
                    self.accumulator -= TICK_DT;
                    self.prev_flyer = self.sim.flyer;
                    let input = self.tick_input();
                    self.sim.step(&input);
                    // The mixer flush is per-tick like the original's
                    // (fade ramps are tick-denominated).
                    self.audio_tick();
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
                let view_pitch = match self.flight.thrust {
                    config::ThrustModel::Mc1 => aim * 0.5,
                    config::ThrustModel::Enhanced => aim,
                };
                let cam = CameraView {
                    x: lerp_wrap(a.x, b.x),
                    y: a.y + (b.y - a.y) * alpha,
                    z: lerp_wrap(a.z, b.z),
                    yaw: a.yaw + (b.yaw - a.yaw) * alpha,
                    pitch: view_pitch - kick,
                    fov_y: FOV_Y,
                };
                // Spell UI quads (book grid or in-flight HUD).
                if let (Some(assets), Some(w)) = (&self.level.ui, &self.sim.world) {
                    let size = self
                        .window
                        .as_ref()
                        .map(|win| win.inner_size())
                        .map(|s| (s.width as f32, s.height as f32))
                        .unwrap_or((1280.0, 720.0));
                    let loadout = w.loadout();
                    let (mut quads, hovered) = if self.book_open() {
                        ui::book_quads(assets, &loadout, size.0, size.1, self.cursor)
                    } else {
                        (ui::hud_quads(assets, &loadout, size.0, size.1), None)
                    };
                    if !self.book_open() {
                        quads.extend(ui::vitals_quads(
                            &w.vitals(),
                            size.0,
                            size.1,
                            (self.sim.tick / 8) % 2 == 0,
                        ));
                    }
                    self.hovered = hovered;
                    if let Some(r) = &mut self.renderer {
                        r.set_ui_quads(quads);
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
            let dy = if self.flight.invert_y { -dy } else { dy };
            if self.flight.thrust == config::ThrustModel::Mc1 {
                // Relative motion integrates into the virtual stick
                // POSITION (the original reads the DOS cursor offset
                // from screen center, clamped ±127 — on a 320-wide
                // screen that's ~0.8 stick units per pixel; modern
                // default trades a little of that for precision).
                let s = STICK_PER_PIXEL * self.flight.mouse_sensitivity;
                self.stick.x = (self.stick.x + dx as f32 * s).clamp(-127.0, 127.0);
                self.stick.y = (self.stick.y - dy as f32 * s).clamp(-127.0, 127.0);
            } else {
                let s = MOUSE_SENSITIVITY * self.flight.mouse_sensitivity;
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
    /// CLI override of `enhancements.smooth_shading`; None = use config.
    smooth_shading: Option<bool>,
    /// CLI override of `enhancements.map_trigger_areas`.
    map_triggers: Option<bool>,
    /// CLI override of `enhancements.health_bars`.
    health_bars: Option<bool>,
    /// CLI override of `enhancements.dev_spells`.
    dev_spells: Option<bool>,
    /// CLI override of `enhancements.invincible`.
    invincible: Option<bool>,
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
    /// Animation clock for `--screenshot` (game turns; default 0).
    /// Water-wave phase repeats every 32 (MC1) / 64 (MC2) turns.
    anim_turn: f32,
    /// Apply the original's load-time terrain features (default true).
    terrain_features: bool,
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
    let mut dev_spells = None;
    let mut invincible = None;
    let mut thrust = None;
    let mut altitude = None;
    let mut bindings = None;
    let mut map = None;
    let mut map_scale = 4u32;
    let mut map_view = false;
    let mut anim_turn = 0.0f32;
    let mut terrain_features = true;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--level" => {
                level = PathBuf::from(it.next().ok_or("--level needs a path")?);
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
            "--dev-spells" => dev_spells = Some(true),
            "--no-dev-spells" => dev_spells = Some(false),
            "--invincible" => invincible = Some(true),
            "--no-invincible" => invincible = Some(false),
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
            "--anim-turn" => {
                anim_turn = it
                    .next()
                    .ok_or("--anim-turn needs a turn count")?
                    .parse()
                    .map_err(|e| format!("--anim-turn: {e}"))?;
            }
            "--no-terrain-features" => terrain_features = false,
            "--help" | "-h" => {
                return Err(format!(
                    "usage: mgcarpet [--level <baked/.../level-NNN.mgcl>] \
                     [--tileset 0|1] [--config <path>] \
                     [--smooth-shading|--no-smooth-shading] \
                     [--map-triggers|--no-map-triggers] \
                     [--dev-spells|--no-dev-spells] \
                     [--invincible|--no-invincible] \
                     [--thrust mc1|enhanced] [--altitude faithful|extended-lift] \
                     [--bindings classic|wasd] \
                     [--screenshot out.png [--camera x,y,z,yaw,pitch] [--map-view] \
                     [--anim-turn N]] \
                     [--map out.png [--map-scale N]] [--no-terrain-features]\n\
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
        dev_spells,
        invincible,
        thrust,
        altitude,
        bindings,
        map,
        map_scale,
        map_view,
        anim_turn,
        terrain_features,
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
    let overlay = mgc_render::MapOverlay {
        dots: level.map_dots.clone(),
        areas: if map_triggers { level.map_areas.clone() } else { Vec::new() },
        stamps: level.map_stamps.clone(),
        path: None,
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

fn run_screenshot(
    mut level: LoadedLevel,
    out: &Path,
    camera: Option<[f32; 5]>,
    smooth_shading: bool,
    map_view: bool,
    anim_turn: f32,
    map_triggers: bool,
    dev_spells: bool,
) -> Result<(), String> {
    let mut renderer = Renderer::offscreen(1280, 720).map_err(|e| e.to_string())?;
    let overlay = mgc_render::MapOverlay {
        dots: level.map_dots.clone(),
        areas: if map_triggers { level.map_areas.clone() } else { Vec::new() },
        stamps: level.map_stamps.clone(),
        path: None,
    };
    renderer.load_level(&level.view, &overlay);
    if let Some((index, atlas)) = &level.sprites {
        renderer.load_sprites(index.clone(), atlas);
    }
    if let Some(assets) = &level.ui {
        renderer.load_ui_atlas(assets.atlas_w, assets.atlas_h, &assets.atlas_rgba);
        if let Ok(p) = std::env::var("MGC_DUMP_UI_ATLAS") {
            write_png(Path::new(&p), assets.atlas_w, assets.atlas_h, &assets.atlas_rgba)?;
        }
    }
    renderer.set_billboards(level.billboards.clone());
    renderer.set_smooth_shading(smooth_shading);
    renderer.set_map_view(map_view);
    renderer.set_anim_turn(anim_turn);
    // Spell UI (book grid or HUD), from the level-start loadout.
    if let (Some(assets), Some(w)) = (&level.ui, &mut level.world) {
        if dev_spells {
            w.set_dev_spells(true);
        }
        let loadout = w.loadout();
        let quads = if map_view {
            ui::book_quads(assets, &loadout, 1280.0, 720.0, (-1.0, -1.0)).0
        } else {
            ui::hud_quads(assets, &loadout, 1280.0, 720.0)
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
    let cfg = match config::Config::load(&config_path, explicit) {
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
    let smooth_shading = args
        .smooth_shading
        .unwrap_or(cfg.enhancements.smooth_shading);
    let map_triggers = args
        .map_triggers
        .unwrap_or(cfg.enhancements.map_trigger_areas);
    let health_bars = args.health_bars.unwrap_or(cfg.enhancements.health_bars);
    let dev_spells = args.dev_spells.unwrap_or(cfg.enhancements.dev_spells);
    let invincible = args.invincible.unwrap_or(cfg.enhancements.invincible);
    let flight = config::FlightConfig {
        thrust: args.thrust.unwrap_or(cfg.flight.thrust),
        altitude: args.altitude.unwrap_or(cfg.flight.altitude),
        bindings: args.bindings.unwrap_or(cfg.flight.bindings),
        mouse_sensitivity: cfg.flight.mouse_sensitivity,
        invert_y: cfg.flight.invert_y,
    };

    let level = match load_level(&args.level, args.tileset, args.terrain_features) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Some(out) = &args.map {
        return match run_map(&level, out, args.map_scale, map_triggers) {
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
            smooth_shading,
            args.map_view,
            args.anim_turn,
            map_triggers,
            dev_spells,
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    println!("mgcarpet {} — {}", env!("CARGO_PKG_VERSION"), level.label);
    let move_keys = match flight.bindings {
        config::Bindings::Classic => "Up/Down arrows accel/decel, Left/Right strafe",
        config::Bindings::Wasd => "W/S accel/decel, A/D strafe",
    };
    match flight.thrust {
        config::ThrustModel::Mc1 => println!(
            "controls: faithful MC1 — mouse = stick (offset steers, recenter to fly straight),\n\
             \x20         {move_keys} (impulses: speed persists until countered),"
        ),
        config::ThrustModel::Enhanced => println!(
            "controls: enhanced — mouse look, {move_keys} (hold-to-fly),"
        ),
    }
    if flight.altitude == config::AltitudeModel::ExtendedLift {
        println!("          E/Q float up/down (extended lift, capped at the highest terrain),");
    }
    println!("          Space respawns after death (at your castle; no castle = level restart),");
    println!("          Shift+L demolishes your own castle one level per press,");
    println!("          LMB/RMB cast the equipped hand's spell (hold = channel),");
    println!("          Enter opens the book: click a spell with LMB/RMB to equip,");
    println!("          hover + 1-9,0 binds a quick key (in flight: equip, Shift = right hand),");
    println!("          T smooth shading, H monster health bars (debug),");
    println!("          G all spells + infinite mana (dev), V map trigger overlay, Esc twice quits");

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("error: cannot create event loop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App::new(
        level,
        smooth_shading,
        map_triggers,
        health_bars,
        dev_spells,
        invincible,
        cfg.enhancements.map_owned_buildings,
        &cfg.audio,
        flight,
    );
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: event loop: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
