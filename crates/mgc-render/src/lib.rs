//! The wgpu renderer.
//!
//! Reads simulation state, never mutates it; interpolates between fixed
//! ticks for smooth motion at any display rate.
//!
//! Design commitments (see project README):
//! - Terrain, billboarded sprites, and water from baked packages.
//! - Palette-index data kept all the way to the fragment shader
//!   (palette-as-LUT) so the authentic 8-bit look is the baseline and
//!   enhanced rendering is a toggle, not a rewrite.

/// Placeholder entry point; the real renderer arrives with the
/// carpet-flyer milestone.
pub fn backend_summary() -> String {
    format!("wgpu {}", env!("CARGO_PKG_VERSION"))
}
