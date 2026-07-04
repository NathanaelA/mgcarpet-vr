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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mgc_formats::{Game, LevelPackage, mgcl};
use mgc_render::{CameraView, LevelView, Renderer};
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
}

/// Resolve the package plus its per-game color assets into what the
/// renderer and sim consume.
fn load_level(level_path: &Path) -> Result<LoadedLevel, String> {
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

    // Assets live in the baked tree next to the per-game level dirs:
    // <baked>/<game>/level-NNN.mgcl and <baked>/mc1/assets/*.bin. MC1
    // and Hidden Worlds share MC1's palette; MC2's palettes are still
    // missing from the game data (see ROADMAP), so MC2 levels borrow
    // the MC1 day palette as a stand-in until the CD files land.
    let baked_root = level_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));
    let assets = baked_root.join("mc1/assets");
    let asset = |name: &str| {
        std::fs::read(assets.join(name))
            .map_err(|e| format!("{}: {e}", assets.join(name).display()))
    };
    let palette_bytes = asset("palette-day.bin")?;
    let tile_colors_bytes = asset("tile-colors.bin")?;
    let shade_lut = asset("shade-lut.bin")?;
    if palette_bytes.len() != 768
        || tile_colors_bytes.len() != 256
        || shade_lut.len() != mgc_render::SHADE_LEVELS * 256
    {
        return Err("malformed palette assets (expected 768 + 256 + 16384 bytes)".into());
    }

    let mut palette = [[0u8; 3]; 256];
    for (i, rgb) in palette.iter_mut().enumerate() {
        rgb.copy_from_slice(&palette_bytes[i * 3..i * 3 + 3]);
    }
    let mut tile_colors = [0u8; 256];
    tile_colors.copy_from_slice(&tile_colors_bytes);

    let game = match package.meta.game {
        Game::MagicCarpet1 => "mc1",
        Game::HiddenWorlds => "mc1hw",
        Game::MagicCarpet2 => "mc2",
    };
    if package.meta.game == Game::MagicCarpet2 {
        eprintln!("note: MC2 palettes not yet baked — using the MC1 day palette as a stand-in");
    }

    Ok(LoadedLevel {
        view: LevelView {
            tile_type: terrain.tile_type.clone(),
            height: terrain.height.clone(),
            shading: terrain.shading.clone(),
            palette,
            tile_colors,
            shade_lut,
        },
        height: terrain.height.clone(),
        label: format!("{game} level {}", package.meta.level),
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
    fn new(level: LoadedLevel) -> Self {
        let sim = Simulation::with_terrain(level.height.clone());
        let prev_flyer = sim.flyer;
        Self {
            level,
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
                renderer.load_level(&self.level.view);
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
}

fn parse_args() -> Result<Args, String> {
    let mut level = PathBuf::from("baked/mc1/level-000.mgcl");
    let mut screenshot = None;
    let mut camera = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--level" => {
                level = PathBuf::from(it.next().ok_or("--level needs a path")?);
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
            "--help" | "-h" => {
                return Err("usage: mgcarpet [--level <baked/.../level-NNN.mgcl>] \
                     [--screenshot out.png [--camera x,y,z,yaw,pitch]]"
                    .into());
            }
            other => return Err(format!("unknown argument {other} (try --help)")),
        }
    }
    Ok(Args {
        level,
        screenshot,
        camera,
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

fn run_screenshot(level: LoadedLevel, out: &Path, camera: Option<[f32; 5]>) -> Result<(), String> {
    let mut renderer = Renderer::offscreen(1280, 720).map_err(|e| e.to_string())?;
    renderer.load_level(&level.view);
    let flyer = Flyer::default();
    let [x, y, z, yaw_deg, pitch_deg] = camera.unwrap_or([flyer.x, flyer.y, flyer.z, 0.0, -11.5]);
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
    let level = match load_level(&args.level) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Some(out) = &args.screenshot {
        return match run_screenshot(level, out, args.camera) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    println!("mgcarpet {} — {}", env!("CARGO_PKG_VERSION"), level.label);
    println!("controls: WASD fly, mouse look (click to grab, Esc to release),");
    println!("          Space/Shift up/down, arrows turn, Esc twice quits");

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("error: cannot create event loop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App::new(level);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: event loop: {e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
