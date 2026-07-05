//! The authenticity-matrix config file.
//!
//! Project stance (README/ROADMAP): the authentic original behavior is
//! always the default, and every modern enhancement is an opt-in flip.
//! This file is where those flips live until a real in-game options
//! screen exists — one field per enhancement, absent file (or field)
//! means fully authentic. CLI flags override the file for one run.
//!
//! Loaded from `mgcarpet.json` in the working directory, or the path
//! given with `--config`. Unknown fields are ignored (older binaries
//! tolerate newer configs).
//!
//! ```json
//! { "enhancements": { "smooth_shading": true } }
//! ```

use std::path::Path;

use serde::Deserialize;

/// Default config file name, looked up in the working directory.
pub const DEFAULT_PATH: &str = "mgcarpet.json";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enhancements: Enhancements,
}

/// Modern-convenience switches, all defaulting to off (= authentic).
/// Grows alongside the roadmap: extended controls, savepoints, ...
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Enhancements {
    /// Interpolate terrain shade across tile centers instead of the
    /// original's one shade level per tile (toggle at runtime with T).
    pub smooth_shading: bool,
}

impl Config {
    /// Load from `path`. When `explicit` is false (the default path),
    /// a missing file simply yields defaults; a path the user asked for
    /// must exist. Malformed JSON is always an error — better loud than
    /// silently authentic.
    pub fn load(path: &Path, explicit: bool) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}
