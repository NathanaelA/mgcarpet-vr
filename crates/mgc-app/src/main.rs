//! The game shell: window and event loop.
//!
//! For now this only opens an empty window and exits cleanly — it proves
//! the winit stack builds and runs on this machine. The wgpu surface and
//! the fixed-timestep loop (sim ticks at `mgc_sim::TICK_RATE_HZ`, render
//! interpolated at display rate) arrive with the carpet-flyer milestone.

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("Magic Carpet");
            match event_loop.create_window(attrs) {
                Ok(window) => self.window = Some(window),
                Err(e) => {
                    eprintln!("error: cannot create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
}

fn main() {
    println!(
        "mgcarpet {} — renderer: {}",
        env!("CARGO_PKG_VERSION"),
        mgc_render::backend_summary()
    );
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("error: cannot create event loop: {e}");
            std::process::exit(1);
        }
    };
    let mut app = App::default();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: event loop: {e}");
        std::process::exit(1);
    }
}
