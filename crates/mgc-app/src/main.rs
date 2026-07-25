//! The desktop `mgcarpet` binary: hand the shell in the `mgc_app`
//! library its default event loop. The split exists for embedding
//! shells that must build their own loop (see `game_main`); passing
//! `None` also keeps the headless modes free of any display-server
//! dependency.

fn main() -> std::process::ExitCode {
    mgc_app::game_main(None)
}
