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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mgc_formats::bundle::Bundle;
use mgc_formats::{Game, LevelPackage, mgcl};
use mgc_render::{Billboard, CameraView, LevelView, Renderer};
use mgc_sim::{FlightInput, Flyer, Simulation, TICK_DT};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const FOV_Y: f32 = 60.0_f32.to_radians();
const MOUSE_SENSITIVITY: f32 = 0.0022;

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
    /// Bundle palette, kept for runtime map-dot rebuilds.
    palette_rgba: [[u8; 4]; 256],
    /// Live trigger/portal volumes for the opt-in map overlay.
    map_areas: Vec<mgc_render::MapArea>,
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
                let w = mgc_sim::world::World::new(
                    mgc_sim::features::Planes {
                        height: height.clone(),
                        tile_type: tile_type.clone(),
                        shading: sh.clone(),
                        angle: an.clone(),
                    },
                    &package.things.things,
                    seed,
                    assets,
                );
                // The view starts from the post-feature planes.
                height.copy_from_slice(&w.planes().height);
                tile_type.copy_from_slice(&w.planes().tile_type);
                shading.as_mut().unwrap().copy_from_slice(&w.planes().shading);
                angle.as_mut().unwrap().copy_from_slice(&w.planes().angle);
                world = Some(w);
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
    // population is its disposition-spawned live set; without one
    // (MC2, --no-terrain-features), every drawable record — the old
    // static behavior, kept as the comparison mode.
    let (billboards, map_dots) = if package.meta.game != Game::MagicCarpet2 {
        let index = bundle.sprites.as_ref().map(|(i, _)| i);
        let live;
        let things: &[mgc_formats::Thing] = match &world {
            Some(w) => {
                live = w.live_things();
                &live
            }
            None => &package.things.things,
        };
        (
            entities::billboards(things, &height, |id| {
                index
                    .and_then(|i| i.sprites.get(id as usize))
                    .map(|s| (s.width, s.height))
            }),
            entities::map_dots(things, &bundle.palette),
        )
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
        palette_rgba: bundle.palette,
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

struct App {
    level: LoadedLevel,
    smooth_shading: bool,
    /// Map trigger-volume overlay (enhancement/debug; V toggles).
    map_triggers: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    sim: Simulation,
    prev_flyer: Flyer,
    keys: HeldKeys,
    mouse: MouseAccum,
    grabbed: bool,
    last_frame: std::time::Instant,
    accumulator: f32,
}

impl App {
    fn new(mut level: LoadedLevel, smooth_shading: bool, map_triggers: bool) -> Self {
        let mut sim = match level.world.take() {
            Some(w) => Simulation::with_world(w),
            None => Simulation::with_terrain(level.height.clone()),
        };
        if let Some(start) = level.start {
            sim.flyer = start;
        }
        let prev_flyer = sim.flyer;
        Self {
            level,
            smooth_shading,
            map_triggers,
            window: None,
            renderer: None,
            sim,
            prev_flyer,
            keys: HeldKeys::default(),
            mouse: MouseAccum::default(),
            grabbed: false,
            last_frame: std::time::Instant::now(),
            accumulator: 0.0,
        }
    }

    fn tick_input(&mut self) -> FlightInput {
        let axis = |neg: bool, pos: bool| (pos as i32 - neg as i32) as f32;
        let k = &self.keys;
        // Keyboard turn rate: radians per tick.
        let key_turn = 2.2 * TICK_DT;
        let input = FlightInput {
            thrust: axis(k.back, k.forward),
            strafe: axis(k.left, k.right),
            lift: axis(k.down, k.up),
            yaw_delta: axis(k.turn_left, k.turn_right) * key_turn + self.mouse.yaw,
            pitch_delta: axis(k.pitch_down, k.pitch_up) * key_turn + self.mouse.pitch,
        };
        self.mouse = MouseAccum::default();
        input
    }

    /// Push runtime world changes (dug terrain, spawned/removed
    /// entities) to the renderer: refresh the level view's planes,
    /// rebuild billboards + map dots, re-upload the plane textures.
    fn sync_world(&mut self) {
        let Some(w) = &mut self.sim.world else { return };
        if !w.terrain_dirty && !w.entities_dirty {
            return;
        }
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
        if w.entities_dirty {
            let live = w.live_things();
            let index = self.level.sprites.as_ref().map(|(i, _)| i);
            self.level.billboards = entities::billboards(&live, &self.level.view.height, |id| {
                index
                    .and_then(|i| i.sprites.get(id as usize))
                    .map(|s| (s.width, s.height))
            });
            self.level.map_dots = entities::map_dots(&live, &self.level.palette_rgba);
            self.level.map_areas = map_areas(w);
        }
        w.terrain_dirty = false;
        w.entities_dirty = false;
        if let Some(r) = &mut self.renderer {
            r.set_billboards(self.level.billboards.clone());
            let areas = if self.map_triggers { &self.level.map_areas[..] } else { &[] };
            r.update_terrain(&self.level.view, &self.level.map_dots, areas);
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
                let areas = if self.map_triggers { &self.level.map_areas[..] } else { &[] };
                renderer.load_level(&self.level.view, &self.level.map_dots, areas);
                if let Some((index, atlas)) = &self.level.sprites {
                    renderer.load_sprites(index.clone(), atlas);
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                if !self.grabbed {
                    self.set_grab(true);
                }
            }
            WindowEvent::Focused(false) => self.set_grab(false),
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
                    }
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
                    if let Some(r) = &mut self.renderer {
                        let areas = if self.map_triggers {
                            &self.level.map_areas[..]
                        } else {
                            &[]
                        };
                        r.update_terrain(&self.level.view, &self.level.map_dots, areas);
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
                let k = &mut self.keys;
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) => k.forward = down,
                    PhysicalKey::Code(KeyCode::KeyS) => k.back = down,
                    PhysicalKey::Code(KeyCode::KeyA) => k.left = down,
                    PhysicalKey::Code(KeyCode::KeyD) => k.right = down,
                    PhysicalKey::Code(KeyCode::Space) => k.up = down,
                    PhysicalKey::Code(KeyCode::ShiftLeft) => k.down = down,
                    PhysicalKey::Code(KeyCode::ArrowLeft) => k.turn_left = down,
                    PhysicalKey::Code(KeyCode::ArrowRight) => k.turn_right = down,
                    PhysicalKey::Code(KeyCode::ArrowUp) => k.pitch_up = down,
                    PhysicalKey::Code(KeyCode::ArrowDown) => k.pitch_down = down,
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
                }
                self.sync_world();

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
                let cam = CameraView {
                    x: lerp_wrap(a.x, b.x),
                    y: a.y + (b.y - a.y) * alpha,
                    z: lerp_wrap(a.z, b.z),
                    yaw: a.yaw + (b.yaw - a.yaw) * alpha,
                    pitch: a.pitch + (b.pitch - a.pitch) * alpha,
                    fov_y: FOV_Y,
                };
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
            self.mouse.yaw += dx as f32 * MOUSE_SENSITIVITY;
            self.mouse.pitch -= dy as f32 * MOUSE_SENSITIVITY;
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
    let areas = if map_triggers { &level.map_areas[..] } else { &[] };
    let src = mgc_render::map_pixels(&level.view, &level.map_dots, areas);
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
    level: LoadedLevel,
    out: &Path,
    camera: Option<[f32; 5]>,
    smooth_shading: bool,
    map_view: bool,
    anim_turn: f32,
    map_triggers: bool,
) -> Result<(), String> {
    let mut renderer = Renderer::offscreen(1280, 720).map_err(|e| e.to_string())?;
    let areas = if map_triggers { &level.map_areas[..] } else { &[] };
    renderer.load_level(&level.view, &level.map_dots, areas);
    if let Some((index, atlas)) = &level.sprites {
        renderer.load_sprites(index.clone(), atlas);
    }
    renderer.set_billboards(level.billboards.clone());
    renderer.set_smooth_shading(smooth_shading);
    renderer.set_map_view(map_view);
    renderer.set_anim_turn(anim_turn);
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
        ) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    println!("mgcarpet {} — {}", env!("CARGO_PKG_VERSION"), level.label);
    println!("controls: WASD fly, mouse look (click to grab, Esc to release),");
    println!("          Space/Shift up/down, arrows turn, T toggles smooth shading,");
    println!("          Enter opens the map (book screen), Esc twice quits");

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("error: cannot create event loop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App::new(level, smooth_shading, map_triggers);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: event loop: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
