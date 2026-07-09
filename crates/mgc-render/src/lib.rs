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
//!
//! Current scope: the terrain pass — a 256x256 tile mesh (one vertex
//! per grid point, engine-authentic alternating diagonals), tiles
//! textured in the fragment shader from the baked terrain atlas (the
//! terrain-type byte is the atlas cell index), texels resolved through
//! the engine's shade LUT and palette; flat map colors as the fallback
//! when no atlas is baked. Per-vertex hillshade, distance fog.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use mgc_sim::{HEIGHT_SCALE, MAP_TILES};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Number of light levels in the engine's shade-remap table.
pub const SHADE_LEVELS: usize = 64;

/// Width in pixels of a baked terrain-texture atlas (`terrain-atlas-N.bin`).
pub const ATLAS_WIDTH: usize = 256;
/// Edge length of one atlas cell (one terrain texture).
pub const ATLAS_CELL: usize = 32;

/// Which game's water-surface animation rule the terrain pass applies
/// (the per-corner sine wave in the original tile projectors; ROADMAP
/// "Terrain water animation").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaveMode {
    /// No animation (static comparison renders).
    #[default]
    Off,
    /// MC1 (remc1 sub_main.cpp:33955): deep-water corners (angle bit 3)
    /// swell by ±¼ tile and shimmer by ±8 shade rows.
    Mc1,
    /// MC2 (remc2 GameRenderOriginal.cpp:1054): every water corner
    /// (type 0) ripples by ±1/32 tile, shimmer gated on shade < 56;
    /// phase advances at half MC1's rate.
    Mc2,
}

/// Everything the renderer needs from a loaded level: terrain arrays
/// from the package, color tables from the baked assets. Pixels resolve
/// exactly like the original engine: a base palette index — an atlas
/// texel where a terrain atlas is available, else the tile's flat map
/// color `tile_colors[type]` — through the shade remap and palette:
/// `palette[shade_lut[shade][index]]`.
pub struct LevelView {
    /// 256x256 terrain-type bytes, row-major `y * 256 + x`.
    pub tile_type: Vec<u8>,
    /// 256x256 height bytes, same layout.
    pub height: Vec<u8>,
    /// 256x256 light levels (the generator's shading array); None for
    /// packages baked without it (a synthetic hillshade fills in).
    pub shading: Option<Vec<u8>>,
    /// 256 RGB triplets (sRGB bytes, as baked).
    pub palette: [[u8; 3]; 256],
    /// Terrain type -> base palette index (`tile-colors-N.bin`).
    pub tile_colors: [u8; 256],
    /// Shade level x base index -> final palette index
    /// (`shade-lut-N.bin`, [`SHADE_LEVELS`] rows of 256).
    pub shade_lut: Vec<u8>,
    /// Terrain-texture atlas (`terrain-atlas-N.bin`): 8-bit palette
    /// indices, [`ATLAS_WIDTH`] wide, [`ATLAS_CELL`]-square cells, the
    /// terrain-type byte indexing cells row-major. None renders every
    /// tile with its flat map color.
    pub atlas: Option<Vec<u8>>,
    /// 256x256 angle/flags bytes (`terrain/angle.bin`): bits 4-6 pick
    /// the tile's texture UV orientation. None renders orientation 0
    /// everywhere (transition tiles like shorelines will misalign).
    pub angle: Option<Vec<u8>>,
    /// The game's water-surface animation rule.
    pub wave: WaveMode,
}

/// One entity dot on the overhead map: tile-unit position and the
/// palette index the original plots for its category.
#[derive(Debug, Clone, Copy)]
pub struct MapDot {
    pub x: f32,
    pub z: f32,
    pub color: u8,
    /// Pixel side length: 1 for the standard dot, 2 for the
    /// original's grown portal dot (sub_48710 v60).
    pub size: u8,
}

/// A tinted circle on the overhead map (the trigger-volume overlay —
/// an opt-in enhancement/debugging aid, never drawn by the original).
/// Tile-unit center and radius, direct RGB (deliberately outside the
/// palette: this layer is explicitly non-faithful).
#[derive(Debug, Clone, Copy)]
pub struct MapArea {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub color: [u8; 3],
}

/// An icon stamped onto the overhead map (the original's castle /
/// balloon UI-sprite markers, remc1 sub_48710 :57224-37). Because both
/// maps are yaw-rotated, stamps must stay SCREEN-UPRIGHT — a flag/
/// balloon always points up regardless of heading — so they are NOT
/// baked into the (rotated) map texture; the renderer projects each
/// one's world position through the map's rotation to a screen rect and
/// blits the upright sprite from the UI atlas. `uv` is the atlas texel
/// rect (x, y, w, h); `w`/`h` double as the on-screen pixel size.
#[derive(Debug, Clone, Copy)]
pub struct MapStamp {
    pub x: f32,
    pub z: f32,
    pub w: u32,
    pub h: u32,
    pub uv: [f32; 4],
    /// Fractional anchor: the point on the sprite (0..1 of its w/h, from
    /// the top-left) that pins to the world position. Per remc1
    /// sub_48710's per-range anchors: castle (58-65) = bottom-LEFT
    /// `(0, 1)` — the flagpole foot; balloon (66-73) = bottom-CENTER
    /// `(0.5, 1)` — the balloon base.
    pub anchor: [f32; 2],
}

/// The marching-ants guide line (remc1 :57161-82): a single-pixel mark
/// every 4 MAP-SURFACE pixels along the screen-projected player→castle
/// line, starting at `(tick & 3) + 4` so the ants march, each plotted
/// through the brighten blend. Drawn screen-space over the rotated map
/// (like the stamps) — NOT baked into the world texture, where the
/// spacing was 4 world TILES and stretched with the radar zoom instead
/// of staying 4px. Endpoints in tile units; `phase` = the 0..3 cycle.
#[derive(Debug, Clone, Copy)]
pub struct MapPath {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub phase: u8,
}

/// Everything baked into the map terrain texture, in draw order: areas
/// (enhancement), then entity dots. (Icon stamps and the guide path
/// draw screen-space at render time — see `Renderer::set_map_stamps` /
/// `Renderer::set_map_path` — so they stay upright/evenly-spaced under
/// rotation.)
#[derive(Debug, Clone, Default)]
pub struct MapOverlay {
    pub dots: Vec<MapDot>,
    pub areas: Vec<MapArea>,
}

/// Flat-color overhead map: one RGBA pixel per tile (256x256, row-major
/// like the terrain grids), each resolved through the engine's map-view
/// color path `palette[shade_lut[shade][tile_colors[type]]]` — the
/// exact lookup the original's fullscreen map uses (remc2 GameUI) —
/// then the (opt-in) area overlay, then entity dots plotted on top, one
/// pixel per entity, exactly like the original (the enhanced marker
/// mode is a planned opt-in).
pub fn map_pixels(level: &LevelView, overlay: &MapOverlay) -> Vec<u8> {
    let n = MAP_TILES;
    let mut out = vec![0u8; n * n * 4];
    for i in 0..n * n {
        let ty = level.tile_type[i] as usize;
        let shade = level
            .shading
            .as_ref()
            .map(|s| (s[i] as usize).min(SHADE_LEVELS - 1))
            .unwrap_or(32);
        let base = level.tile_colors[ty] as usize;
        let idx = level.shade_lut[shade * 256 + base] as usize;
        out[i * 4..i * 4 + 3].copy_from_slice(&level.palette[idx]);
        out[i * 4 + 3] = 255;
    }
    // Area overlay: a light tint fill with a stronger rim, wrapping
    // toroidally like everything else on the map.
    for a in &overlay.areas {
        let r = a.radius.max(0.5);
        let (cx, cz) = (a.x, a.z);
        let span = r.ceil() as i32;
        for dz in -span..=span {
            for dx in -span..=span {
                let d = ((dx * dx + dz * dz) as f32).sqrt();
                if d > r + 0.5 {
                    continue;
                }
                let blend = if d > r - 1.0 { 0.75 } else { 0.30 };
                let x = (cx as i32 + dx).rem_euclid(n as i32) as usize;
                let z = (cz as i32 + dz).rem_euclid(n as i32) as usize;
                let i = (z * n + x) * 4;
                for c in 0..3 {
                    let base = out[i + c] as f32;
                    out[i + c] = (base + (a.color[c] as f32 - base) * blend) as u8;
                }
            }
        }
    }
    // NOTE: the marching-ants guide path is NOT baked here — retail
    // steps it in MAP-SURFACE pixels along the projected line
    // (:57161-82), so it draws screen-space with the stamps (see
    // `project_guide_path`); baking it stepped in world tiles read
    // ~1.5× sparser on the book map and stretched with radar zoom.
    for dot in &overlay.dots {
        let x = (dot.x as usize).min(n - 1);
        let z = (dot.z as usize).min(n - 1);
        // `size` covers the original's 2x2 grown dot (portals).
        for dz in 0..dot.size as usize {
            for dx in 0..dot.size as usize {
                let i = ((z + dz) % n) * n + (x + dx) % n;
                out[i * 4..i * 4 + 3].copy_from_slice(&level.palette[dot.color as usize]);
                out[i * 4 + 3] = 255;
            }
        }
    }
    // NOTE: icon stamps (castle/balloon) are NOT baked here — they must
    // stay screen-upright under map rotation, so the renderer projects
    // and blits them as upright screen-space quads after the rotated
    // map draw (see `Renderer::map_stamp_quads`).
    out
}

/// Clip a textured quad to `bounds` (both `[x, y, w, h]` pixels),
/// trimming the atlas `uv` rect proportionally so the visible texels
/// stay put — the retail DrawBitmap clips marker sprites at the map
/// window's edge the same way. None when nothing remains.
fn clip_quad_to(rect: [f32; 4], uv: [f32; 4], bounds: [f32; 4]) -> Option<([f32; 4], [f32; 4])> {
    let x0 = rect[0].max(bounds[0]);
    let y0 = rect[1].max(bounds[1]);
    let x1 = (rect[0] + rect[2]).min(bounds[0] + bounds[2]);
    let y1 = (rect[1] + rect[3]).min(bounds[1] + bounds[3]);
    if x1 <= x0 || y1 <= y0 || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return None;
    }
    let fx = (x0 - rect[0]) / rect[2];
    let fy = (y0 - rect[1]) / rect[3];
    let fw = (x1 - x0) / rect[2];
    let fh = (y1 - y0) / rect[3];
    Some((
        [x0, y0, x1 - x0, y1 - y0],
        [
            uv[0] + uv[2] * fx,
            uv[1] + uv[3] * fy,
            uv[2] * fw,
            uv[3] * fh,
        ],
    ))
}

/// Project map stamps onto one map surface as upright screen-space
/// quads — the pure core of [`Renderer::map_stamp_quads`], mirroring
/// map.wgsl's sampling transform (inverted): a stamp at world delta
/// `d` from the player lands at pane offset `R(-yaw)·d` scaled so
/// `zoom/2` tiles fill the pane half-extent (shorter axis; the longer
/// axis stretches by `aspect` exactly as the shader does).
///
/// TOROIDAL VISIBILITY: the world tiles every 256 tiles on both axes
/// and the shader samples it with `fract()`, so a stamp is visible
/// wherever ANY wrapped image of it lands on the pane. Wrapping the
/// delta per-axis BEFORE rotation and testing after loses diagonal
/// images — a (+100,+100) delta rotated 45° lands 141 tiles out, off a
/// 128-half-tile pane, while the map texture still shows that spot via
/// the wrap (the icon blinked out at diagonal headings). Testing each
/// candidate image (±256 per axis) fixes it; edge positions may
/// legitimately draw twice, matching the texture repeat.
///
/// The anchor POINT must land on the surface (retail only marks
/// entities whose map position is inside the window); the sprite rect
/// is then clipped to the surface bounds — for the round radar, to the
/// disc's bounding square (the rim corners can still bleed a few px; a
/// per-pixel disc mask can ride along with the LUT-bake pass if it
/// reads badly in play).
#[allow(clippy::too_many_arguments)]
fn project_map_stamps(
    stamps: &[MapStamp],
    cx: f32,
    cy: f32,
    half_x: f32,
    half_y: f32,
    px: f32,
    pz: f32,
    yaw: f32,
    zoom: f32,
    round: bool,
    aspect: f32,
    scale: f32,
) -> Vec<UiQuad> {
    let half_tiles = zoom * 0.5;
    let tiles = MAP_TILES as f32;
    // Match the shader: screen-up (-y) maps to "ahead"; the sample is
    // world = player + (off.x·cos + off.y·sin, off.x·sin − off.y·cos).
    // The forward map [c s; s −c] is an involution, so the same matrix
    // maps world delta → the shader's centered coords `p`.
    let (s, c) = yaw.sin_cos();
    let bounds = [cx - half_x, cy - half_y, half_x * 2.0, half_y * 2.0];
    let mut quads = Vec::new();
    for st in stamps {
        // Base image in [0, tiles); the −tiles sibling per axis covers
        // every offset a ≤full-world (stretched ≤~1.42×half) pane can
        // show.
        let bx = (st.x - px).rem_euclid(tiles);
        let bz = (st.z - pz).rem_euclid(tiles);
        for dx in [bx, bx - tiles] {
            for dz in [bz, bz - tiles] {
                let ox = dx * c + dz * s;
                let oy = dx * s - dz * c;
                // Tiles → pane-normalized [-1,1]. The shader's `p.y` is
                // NDC (y-UP), UiQuad space is y-DOWN — the flip keeps
                // stamps co-rotating with the map. The shader stretches
                // the LONGER axis's world span by `aspect`; mirror that.
                let mut nx = ox / half_tiles;
                let mut ny = -oy / half_tiles;
                if aspect >= 1.0 {
                    nx /= aspect;
                } else {
                    ny *= aspect;
                }
                if round && (nx * nx + ny * ny) > 1.0 {
                    continue;
                }
                if nx.abs() > 1.0 || ny.abs() > 1.0 {
                    continue;
                }
                let scx = cx + nx * half_x;
                let scy = cy + ny * half_y;
                let (w, h) = (st.w as f32 * scale, st.h as f32 * scale);
                // Per-stamp anchor (remc1 sub_48710 :57344-64): the
                // world point pins to `anchor`·(w,h) from the top-left
                // — castle (58-65) bottom-LEFT `DrawBitmap(v41,
                // v42−h)`, balloon (66-73) bottom-CENTER `(v41−w/2,
                // v42−h)`. uv is atlas texels (ui.wgsl divides).
                let rect = [scx - st.anchor[0] * w, scy - st.anchor[1] * h, w, h];
                if let Some((rect, uv)) = clip_quad_to(rect, st.uv, bounds) {
                    quads.push(UiQuad {
                        rect,
                        uv,
                        tint: [1.0, 1.0, 1.0, 1.0],
                    });
                }
            }
        }
    }
    quads
}

/// The marching-ants guide path on one map surface (remc1 :57155-82),
/// as screen-space single-"pixel" quads. Retail projects the
/// player→target line onto the map surface and plots a mark every 4
/// SURFACE pixels starting at `(tick & 3) + 4` (the march), breaking at
/// the surface/window edge — the spacing is constant on screen no
/// matter the zoom. `scale` = screen px per native surface px; marks
/// are `scale`-sized like the surface's own pixels. The mark ink is
/// the blend-LUT brighten (byte_BB934 toward byte_AE167) — a
/// translucent near-white until the LUT bake.
#[allow(clippy::too_many_arguments)]
fn project_guide_path(
    path: &MapPath,
    cx: f32,
    cy: f32,
    half_x: f32,
    half_y: f32,
    px: f32,
    pz: f32,
    yaw: f32,
    zoom: f32,
    round: bool,
    aspect: f32,
    scale: f32,
) -> Vec<UiQuad> {
    const ANT_INK: [f32; 4] = [1.0, 1.0, 0.95, 0.8];
    let half_tiles = zoom * 0.5;
    let tiles = MAP_TILES as f32;
    let (s, c) = yaw.sin_cos();
    // Project a world point to pane pixels (no cull) — the same
    // transform as the stamps, shortest-wrap image of the delta.
    let project = |wx: f32, wz: f32| -> (f32, f32) {
        let dx = (wx - px + tiles * 0.5).rem_euclid(tiles) - tiles * 0.5;
        let dz = (wz - pz + tiles * 0.5).rem_euclid(tiles) - tiles * 0.5;
        let ox = dx * c + dz * s;
        let oy = dx * s - dz * c;
        let mut nx = ox / half_tiles;
        let mut ny = -oy / half_tiles;
        if aspect >= 1.0 {
            nx /= aspect;
        } else {
            ny *= aspect;
        }
        (cx + nx * half_x, cy + ny * half_y)
    };
    let (fx, fy) = project(path.from.0, path.from.1);
    let (tx, ty) = project(path.to.0, path.to.1);
    let (dx, dy) = (tx - fx, ty - fy);
    let dist = (dx * dx + dy * dy).sqrt();
    let mut quads = Vec::new();
    if dist < 1.0 || scale <= 0.0 {
        return quads;
    }
    let (ux, uy) = (dx / dist, dy / dist);
    let dot = scale.max(1.0);
    // March in native surface pixels: start (phase & 3) + 4, step 4
    // (:57161), breaking at the first mark off the surface/disc like
    // retail's bounds checks.
    let mut t = ((path.phase & 3) as f32 + 4.0) * scale;
    let step = 4.0 * scale;
    while t <= dist {
        let (mx, my) = (fx + ux * t, fy + uy * t);
        let (nx, ny) = ((mx - cx) / half_x, (my - cy) / half_y);
        if nx.abs() > 1.0 || ny.abs() > 1.0 {
            break;
        }
        if round && (nx * nx + ny * ny) > 1.0 {
            break;
        }
        quads.push(UiQuad {
            rect: [mx, my, dot, dot],
            uv: [0.0, 0.0, 0.0, 0.0], // solid quad (uv.z == 0)
            tint: ANT_INK,
        });
        t += step;
    }
    quads
}

/// Camera state for one rendered frame (already interpolated).
#[derive(Debug, Clone, Copy)]
pub struct CameraView {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    pos: [f32; 3],
    light: f32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    fog_color: [f32; 4],
    /// x = atlas cell count (0 = untextured), y/z/w reserved.
    atlas: [u32; 4],
    /// Camera basis for billboard expansion (screen-aligned quads).
    cam_right: [f32; 4],
    cam_up: [f32; 4],
}

/// One world sprite to draw, resolved from a level entity. Static data;
/// the view-dependent part (which rotation view, mirroring) is computed
/// per frame from `yaw` and the camera.
#[derive(Debug, Clone, Copy)]
pub struct Billboard {
    /// Feet-center position, world units (x/z tile coords, y altitude).
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Facing, radians (same convention as [`CameraView::yaw`]).
    pub yaw: f32,
    /// First sprite id of the entity's view/animation family.
    pub sprite_base: u16,
    /// The original's view-selection mode (sprite flags high byte /
    /// stats-table draw type): 0/1/21 single view, 2..=16 animation,
    /// 17 = 8 views + mirrored back half, 18 = 16 views, 19/20 =
    /// 5-/3-view folds.
    pub draw_type: u8,
    /// Per-entity animation byte (entity offset 88): for the 2..=16
    /// animation draw types the original draws sprite `base + frame`.
    /// 0 for static/rotation-view entities.
    pub frame: u8,
    /// World height of the quad (engine `var_8 / 256`).
    pub world_h: f32,
}

/// 16 view sectors folded to 5 sprites (draw type 19, `byte_906E8`).
const VIEW_FOLD_5: [u8; 16] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 2, 2, 1, 1, 0];
/// 16 view sectors folded to 3 sprites (draw type 20, `byte_906F8`).
const VIEW_FOLD_3: [u8; 16] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, 0, 0];

/// One monster health bar (unfaithful debug overlay): the classic
/// red-on-black rectangle floating above the sprite.
#[derive(Debug, Clone, Copy)]
pub struct HealthBar {
    /// Bar bottom-center, world units (x/z tile coords, y altitude).
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Bar width in world units.
    pub w: f32,
    /// Remaining life fraction 0..=1.
    pub frac: f32,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BarInstance {
    pos: [f32; 3],
    size: [f32; 2],
    frac: f32,
}

/// One screen-space UI quad (spellbook icon, HUD slot, bar fill).
/// Pixel coordinates, origin top-left. `uv` addresses the RGBA UI
/// atlas in texels; a zero-width uv marks a solid quad drawn from
/// `tint` alone. Tint multiplies sampled color (dim = grey tint).
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct UiQuad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub tint: [f32; 4],
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BillboardInstance {
    pos: [f32; 3],
    size: [f32; 2],
    uv_pos: [f32; 2],
    uv_size: [f32; 2],
    /// x = mirror, y = shade LUT row.
    flags: [u32; 2],
    _pad: [u32; 1],
}

/// Sky/fog color, the classic hazy horizon. sRGB values converted to
/// linear where uploaded.
const SKY_SRGB: [f32; 3] = [0.42, 0.55, 0.75];
const FOG_DENSITY: f32 = 0.006;

// Both maps are player-centered, yaw-rotated and toroidally wrapping
// (player directive 2026-07-07). World spans derive from the original's
// DrawMinimap_49300 params: span_tiles = a6 * a8 / a5 / 256 (BYTE1 tile
// step; hi-res halves a5/a6 and doubles a8, cancelling).
//
/// Book-screen (Enter) map zoom. The original passes 382/378/a8=170 →
/// ~251 tiles, JUST short of the 256-tile world, which is why its edges
/// clip ("questionable things at the edges"). Our deliberate
/// improvement: span the FULL world so nothing is cut. Toroidal wrap
/// makes it appear infinite (the original's rounding-error void-mobs
/// live at that wrap; we don't reproduce those).
const BOOK_MAP_ZOOM: f32 = MAP_TILES as f32;
// The book/map screen topology (sub_20E60 case 4 + the spellbook grid
// at :26915), in the original's hi-res 640×480 native coordinates,
// scaled to the live resolution by w/640, hpx/480. The live world fills
// the background; the map pane and spellbook overlay it, leaving the
// world visible in the top-right L-remainder and the bottom log strip.
/// The book map pane: `DrawMinimap(0,0, 382,378, ...)` at the top-left
/// corner (native px).
const BOOK_MAP_X: f32 = 0.0;
const BOOK_MAP_Y: f32 = 0.0;
// Book/map screen native geometry, MEASURED from the player's hi-res
// retail screenshot (2026-07-07), which is senior over the decompile's
// raw DrawMinimap args (382×378 was the sample size, not the on-screen
// pane). Layout: map pane top-left, world viewport top-right, spellbook
// bottom-right, ~64px black bar along the bottom. There is a 2px BLACK
// GAP forming a "T" between the three panes — taken out of the MAP and
// the LIVE VIEW, NOT the spellbook (which is 1:1 to retail). player
// 2026-07-07.
//   spellbook:  x 384..640, y 194..416 (4 cols × 6 rows of 64×37) — FIXED
//   map:        (0,0) (384−GAP) × 416   [right edge recedes for the gap]
//   viewport:   x 384..640, y 0..(194−GAP)   [bottom recedes for the gap]
//   bottom bar: y 416..480 (black)
/// The 2px black demarcation between the book panes (native px).
const BOOK_GAP: f32 = 2.0;
const BOOK_MAP_W: f32 = 384.0 - BOOK_GAP;
const BOOK_MAP_H: f32 = 416.0;
/// The spellbook grid origin (native px): 24 spells in 4 cols × 6 rows
/// of the slot-slab [3] = 64×37, tightly packed from (384,194). FIXED —
/// the gap is taken from the map/viewport, not here. The grid is drawn
/// app-side (`ui::book_quads` consumes these same constants — ONE
/// source for the measured layout); the renderer needs the LEFT + TOP
/// to place the world viewport.
pub const BOOK_SPELL_X: f32 = 384.0;
pub const BOOK_SPELL_Y: f32 = 194.0;
// The HUD top strip is six tiles packed left-to-right from x=2 with 0px
// gaps (player pixel-measurements 2026-07-07, matched to native sprite
// widths at scale 1.668): [40] radar frame (124) | three [41] sub-panels
// (128 each) | two spell frames [1]/[2] (64 each). Native tile origins:
// 2, 126, 254, 382, 510, 574.
/// In-flight radar: the disc is anchored at the screen CORNER (0,0) and
/// spans the full 128 native px — it touches both edges with NO margin
/// (retail: DrawMinimap(0,0,128,128,...); the [40] frame sprite is what
/// leaves the visible margin, drawn on top). So the disc is slightly
/// bigger than its frame tile, and radar objects read slightly larger.
/// Native px, scaled by w/640 to track the panels. Zoom stays FAITHFUL
/// at 128 tiles across; `+`/`-` adjust it at runtime.
const MINIMAP_DIAM: f32 = 128.0;
const MINIMAP_ZOOM: f32 = 128.0;
/// HUD transparency alpha (radar + panels; kept in sync with ui.rs's
/// PANEL_TINT). The whole HUD blends over the sky in faithful MC1.
pub const HUD_PANEL_ALPHA: f32 = 0.62;
/// Runtime radar-zoom bounds (`+`/`-`): from a tight 32-tile crop out
/// to a near-whole-world 224 tiles.
const MINIMAP_ZOOM_MIN: f32 = 32.0;
const MINIMAP_ZOOM_MAX: f32 = 224.0;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn sky_color_linear() -> [f64; 3] {
    [
        srgb_to_linear(SKY_SRGB[0]) as f64,
        srgb_to_linear(SKY_SRGB[1]) as f64,
        srgb_to_linear(SKY_SRGB[2]) as f64,
    ]
}

enum Target {
    Window {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        color: wgpu::Texture,
        width: u32,
        height: u32,
    },
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    depth: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
    vertex_buf: Option<wgpu::Buffer>,
    index_buf: Option<wgpu::Buffer>,
    index_count: u32,
    /// Cell count of the loaded terrain atlas (0 = render flat colors).
    atlas_cells: u32,
    /// The level's water-wave rule, as a shader selector (0/1/2).
    wave_mode: u32,
    /// Animation clock in original game turns (fractional between
    /// ticks); drives the water wave and sprite frame cycling.
    anim_turn: f32,
    /// Interpolate per-tile shade across tile centers (enhancement,
    /// off = the original's per-tile shade snap).
    smooth_shading: bool,
    /// The book screen (the original's Enter view): overhead map on the
    /// right half, left half reserved for the spell list.
    map_view: bool,
    map_pipeline: wgpu::RenderPipeline,
    map_globals_buf: wgpu::Buffer,
    map_bind_group_layout: wgpu::BindGroupLayout,
    map_bind_group: Option<wgpu::BindGroup>,
    /// The in-flight round minimap (top-left corner): its own uniform +
    /// bind group over the SAME world map texture, drawn during normal
    /// flight (the book screen uses `map_bind_group`). None until a
    /// level is loaded (which is what gates the draw).
    minimap_globals_buf: wgpu::Buffer,
    minimap_bind_group: Option<wgpu::BindGroup>,
    /// Runtime radar zoom (tiles across the disc); `+`/`-` adjust it.
    minimap_zoom: f32,
    /// Radar output alpha — HUD transparency (1 = opaque; the MC1
    /// default matches the translucent panels, MC2/opaque = 1).
    minimap_alpha: f32,
    fill_pipeline: wgpu::RenderPipeline,
    // Billboard (world sprite) pass.
    billboard_pipeline: wgpu::RenderPipeline,
    billboard_bind_group_layout: wgpu::BindGroupLayout,
    billboard_bind_group: Option<wgpu::BindGroup>,
    billboard_buf: Option<wgpu::Buffer>,
    billboard_capacity: usize,
    /// CPU copy of the sprite index for per-frame view selection.
    sprite_index: Option<mgc_formats::bundle::SpriteIndex>,
    sprite_tex: Option<wgpu::Texture>,
    colormap_tex: Option<wgpu::Texture>,
    billboards: Vec<Billboard>,
    // Health-bar overlay pass (unfaithful debug enhancement).
    bar_pipeline: wgpu::RenderPipeline,
    bar_bind_group: wgpu::BindGroup,
    bar_buf: Option<wgpu::Buffer>,
    bar_capacity: usize,
    health_bars: Vec<HealthBar>,
    // Screen-space UI pass (spellbook / HUD).
    ui_pipeline: wgpu::RenderPipeline,
    ui_bind_group_layout: wgpu::BindGroupLayout,
    ui_globals_buf: wgpu::Buffer,
    ui_bind_group: Option<wgpu::BindGroup>,
    ui_buf: Option<wgpu::Buffer>,
    ui_capacity: usize,
    ui_quads: Vec<UiQuad>,
    /// Upright screen-space map icons (castle/balloon), projected onto
    /// whichever map surface is active each frame. World-positioned but
    /// drawn unrotated so they always point up.
    map_stamps: Vec<MapStamp>,
    /// The marching-ants guide path (player → castle), projected onto
    /// the active map surface each frame in 4-surface-px steps.
    map_path: Option<MapPath>,
    /// UI atlas dimensions, needed to convert stamp texel UVs. Set by
    /// `load_ui_atlas`.
    ui_atlas_size: (u32, u32),
    /// Terrain plane textures [type, shade, angle, height] kept for
    /// runtime updates (craters, quakes — `update_terrain`).
    plane_texs: Option<[wgpu::Texture; 4]>,
    /// Overhead map texture, rewritten when terrain/entities change.
    map_tex: Option<wgpu::Texture>,
}

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    Device(String),
    Surface(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "no compatible GPU adapter found"),
            Self::Device(e) => write!(f, "device: {e}"),
            Self::Surface(e) => write!(f, "surface: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl Renderer {
    /// Renderer presenting to a winit window.
    pub fn for_window(window: Arc<winit::window::Window>) -> Result<Self, RenderError> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let (adapter, device, queue) = request_device(&instance, Some(&surface))?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or(RenderError::NoAdapter)?;
        // Prefer an sRGB format so shader output is linear color.
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = *srgb;
        }
        surface.configure(&device, &config);
        let format = config.format;
        let (width, height) = (config.width, config.height);
        Ok(Self::finish_init(
            device,
            queue,
            Target::Window { surface, config },
            format,
            width,
            height,
        ))
    }

    /// Renderer drawing into an offscreen texture (screenshot mode,
    /// used for autonomous end-to-end verification).
    pub fn offscreen(width: u32, height: u32) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let (_adapter, device, queue) = request_device(&instance, None)?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self::finish_init(
            device,
            queue,
            Target::Offscreen {
                color,
                width,
                height,
            },
            format,
            width,
            height,
        ))
    }

    fn finish_init(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Tile types and shading feed the vertex stage too (the
                // per-corner water-wave gates), like the angle plane.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Height plane (vertex-stage altitude; runtime terrain
                // mutation rewrites it).
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // The map (book screen) pass: fullscreen-quad pipeline over the
        // CPU-composed map texture.
        let map_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map"),
            source: wgpu::ShaderSource::Wgsl(include_str!("map.wgsl").into()),
        });
        let map_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("map"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let map_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map"),
            bind_group_layouts: &[&map_bind_group_layout],
            push_constant_ranges: &[],
        });
        let map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map"),
            layout: Some(&map_layout),
            vertex: wgpu::VertexState {
                module: &map_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &map_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Alpha blend so the in-flight radar can be
                    // translucent (HUD transparency); the book map and
                    // opaque-HUD radar pass alpha = 1 (a no-op blend).
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let map_globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map globals"),
            size: 48, // 3 vec4: rect, player(x,z,yaw,zoom), mode(round,aspect,_,_)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let minimap_globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("minimap globals"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Solid sky fill behind the book screen's world viewport.
        let fill_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fill.wgsl").into()),
        });
        let fill_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fill"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill"),
            layout: Some(&fill_layout),
            vertex: wgpu::VertexState {
                module: &fill_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fill_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Billboard pass: instanced screen-aligned quads over the
        // sprite atlas, same colormap as terrain.
        let billboard_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("billboard"),
            source: wgpu::ShaderSource::Wgsl(include_str!("billboard.wgsl").into()),
        });
        let billboard_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("billboard"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let billboard_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("billboard"),
            bind_group_layouts: &[&billboard_bind_group_layout],
            push_constant_ranges: &[],
        });
        let billboard_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("billboard"),
            layout: Some(&billboard_layout),
            vertex: wgpu::VertexState {
                module: &billboard_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BillboardInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x2, 2 => Float32x2,
                        3 => Float32x2, 4 => Uint32x2,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &billboard_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Health-bar overlay: solid-color instanced quads on the same
        // camera basis; own single-binding layout so bars draw even
        // before any sprite atlas is loaded.
        let bar_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bar"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bar.wgsl").into()),
        });
        let bar_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bar"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let bar_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bar"),
            layout: &bar_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        let bar_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bar"),
            bind_group_layouts: &[&bar_bind_group_layout],
            push_constant_ranges: &[],
        });
        let bar_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bar"),
            layout: Some(&bar_layout),
            vertex: wgpu::VertexState {
                module: &bar_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BarInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x2, 2 => Float32,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bar_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Screen-space UI pass (spellbook / HUD): pixel-space textured
        // quads over an RGBA atlas the app pre-composites through the
        // engine's blend LUT. Alpha-blended, no depth.
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let ui_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ui"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });
        let ui_globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[&ui_bind_group_layout],
            push_constant_ranges: &[],
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&ui_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let depth = create_depth(&device, width, height);

        Self {
            device,
            queue,
            target,
            depth,
            pipeline,
            globals_buf,
            bind_group_layout,
            bind_group: None,
            vertex_buf: None,
            index_buf: None,
            index_count: 0,
            atlas_cells: 0,
            wave_mode: 0,
            anim_turn: 0.0,
            plane_texs: None,
            map_tex: None,
            smooth_shading: false,
            map_view: false,
            map_pipeline,
            map_globals_buf,
            minimap_globals_buf,
            minimap_bind_group: None,
            minimap_zoom: MINIMAP_ZOOM,
            minimap_alpha: 1.0,
            map_bind_group_layout,
            map_bind_group: None,
            fill_pipeline,
            billboard_pipeline,
            billboard_bind_group_layout,
            billboard_bind_group: None,
            billboard_buf: None,
            billboard_capacity: 0,
            sprite_index: None,
            sprite_tex: None,
            colormap_tex: None,
            billboards: Vec::new(),
            bar_pipeline,
            bar_bind_group,
            bar_buf: None,
            bar_capacity: 0,
            health_bars: Vec::new(),
            ui_pipeline,
            ui_bind_group_layout,
            ui_globals_buf,
            ui_bind_group: None,
            ui_buf: None,
            ui_capacity: 0,
            ui_quads: Vec::new(),
            map_stamps: Vec::new(),
            map_path: None,
            ui_atlas_size: (1, 1),
        }
    }

    /// Toggle the book screen (overhead map + reserved spell half).
    pub fn set_map_view(&mut self, on: bool) {
        self.map_view = on;
    }

    pub fn map_view(&self) -> bool {
        self.map_view
    }

    /// Toggle smooth (tile-interpolated) shading; off is the original's
    /// per-tile shade snap. Takes effect on the next frame.
    pub fn set_smooth_shading(&mut self, on: bool) {
        self.smooth_shading = on;
    }

    pub fn smooth_shading(&self) -> bool {
        self.smooth_shading
    }

    /// Advance the animation clock, in original game turns (one sim
    /// tick = one turn; pass a fractional part for render
    /// interpolation). Drives the water wave and the sprite frame
    /// cycling; both repeat within 4096 turns, so callers should wrap
    /// (`tick % 4096`) to keep f32 precision over long sessions.
    pub fn set_anim_turn(&mut self, turn: f32) {
        self.anim_turn = turn;
    }

    /// Set the radar's HUD transparency: `true` = translucent (faithful
    /// MC1, matches the panels), `false` = opaque (MC2 readability
    /// toggle). Alpha kept in sync with the panel alpha in ui.rs.
    pub fn set_hud_transparent(&mut self, transparent: bool) {
        self.minimap_alpha = if transparent { HUD_PANEL_ALPHA } else { 1.0 };
    }

    /// Multiply the radar zoom (tiles across the disc), clamped to a
    /// sane range. `factor` < 1 zooms in (fewer tiles), > 1 zooms out.
    /// Bound to `+`/`-` in the app (MC2/MC1 runtime radar zoom).
    pub fn zoom_minimap(&mut self, factor: f32) {
        self.minimap_zoom = (self.minimap_zoom * factor).clamp(MINIMAP_ZOOM_MIN, MINIMAP_ZOOM_MAX);
    }

    pub fn minimap_zoom(&self) -> f32 {
        self.minimap_zoom
    }

    /// Upload a level: build the terrain mesh, the color/type LUTs, and
    /// the overhead map (terrain + entity dots).
    pub fn load_level(&mut self, level: &LevelView, overlay: &MapOverlay) {
        let n = MAP_TILES;
        assert_eq!(level.height.len(), n * n);
        assert_eq!(level.tile_type.len(), n * n);
        self.wave_mode = match level.wave {
            WaveMode::Off => 0,
            WaveMode::Mc1 => 1,
            WaveMode::Mc2 => 2,
        };

        // Height at a wrapped grid point.
        let h = |x: usize, z: usize| -> f32 {
            level.height[(z % n) * n + (x % n)] as f32 * HEIGHT_SCALE
        };

        // One vertex per grid point, plus a duplicated wrap row/column so
        // the last tile closes the seam with the first.
        let verts_per_side = n + 1;
        let mut vertices = Vec::with_capacity(verts_per_side * verts_per_side);
        // When the package carries the generator's shading array, it is
        // the light source (vertex light stays 1.0). Otherwise fall back
        // to a synthetic hillshade: fixed sun from the north-west.
        let synthetic = level.shading.is_none();
        let sun = {
            let v: [f32; 3] = [-0.45, 0.8, -0.4];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / len, v[1] / len, v[2] / len]
        };
        for z in 0..verts_per_side {
            for x in 0..verts_per_side {
                let y = h(x, z);
                let light = if synthetic {
                    // Central-difference normal with wraparound neighbors.
                    let dx = h(x + 1, z) - h(x + n - 1, z);
                    let dz = h(x, z + 1) - h(x, z + n - 1);
                    let inv = 1.0 / (dx * dx + dz * dz + 4.0).sqrt();
                    let normal = [-dx * inv, 2.0 * inv, -dz * inv];
                    let ndotl = normal[0] * sun[0] + normal[1] * sun[1] + normal[2] * sun[2];
                    0.55 + 0.55 * ndotl.max(0.0)
                } else {
                    1.0
                };
                // y stays 0 in the buffer: the vertex shader reads the
                // height plane texture so runtime terrain mutation is
                // a texture write, not a mesh rebuild.
                let _ = y;
                vertices.push(Vertex {
                    pos: [x as f32, 0.0, z as f32],
                    light,
                });
            }
        }

        // Two triangles per tile; diagonal orientation alternates in a
        // checkerboard exactly like the engine's altitude interpolation
        // (sub_B5C60: `(tile_x + tile_z) & 1` picks the split).
        let mut indices: Vec<u32> = Vec::with_capacity(n * n * 6);
        let at = |x: usize, z: usize| (z * verts_per_side + x) as u32;
        for z in 0..n {
            for x in 0..n {
                let (a, b, c, d) = (at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1));
                if (x + z) & 1 == 0 {
                    // Split along the a-c diagonal.
                    indices.extend_from_slice(&[a, c, b, a, d, c]);
                } else {
                    // Split along the b-d diagonal.
                    indices.extend_from_slice(&[a, d, b, b, d, c]);
                }
            }
        }

        use wgpu::util::DeviceExt;
        self.vertex_buf = Some(
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("terrain vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
        );
        self.index_buf = Some(
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("terrain indices"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
        );
        self.index_count = indices.len() as u32;

        // A small helper: 2D R8Uint texture from a byte grid.
        let byte_tex = |label: &str, bytes: &[u8], width: u32, height: u32| {
            let extent = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                tex.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: None,
                },
                extent,
            );
            tex
        };

        let type_tex = byte_tex("tile types", &level.tile_type, n as u32, n as u32);
        // Without a baked shading array, a constant mid level keeps the
        // colormap row selection stable (vertex light shades instead).
        let flat_shading;
        let shading: &[u8] = match &level.shading {
            Some(s) => s,
            None => {
                flat_shading = vec![32u8; n * n];
                &flat_shading
            }
        };
        let shade_tex = byte_tex("tile shading", shading, n as u32, n as u32);

        // Type -> flat base palette index, for tiles rendered without a
        // texture (no atlas, or type beyond the atlas).
        let tile_colors_tex = byte_tex("tile colors", &level.tile_colors, 256, 1);

        // Terrain-texture atlas (a 1x1 dummy keeps the bind group layout
        // uniform when the level has none; the shader gates on the cell
        // count in Globals).
        let (atlas_data, atlas_w, atlas_h): (&[u8], u32, u32) = match &level.atlas {
            Some(a) => {
                assert_eq!(a.len() % (ATLAS_WIDTH * ATLAS_CELL), 0, "ragged atlas");
                (a, ATLAS_WIDTH as u32, (a.len() / ATLAS_WIDTH) as u32)
            }
            None => (&[0], 1, 1),
        };
        self.atlas_cells = level
            .atlas
            .as_ref()
            .map(|a| (a.len() / (ATLAS_WIDTH * ATLAS_CELL)) * (ATLAS_WIDTH / ATLAS_CELL))
            .unwrap_or(0) as u32;
        let atlas_tex = byte_tex("terrain atlas", atlas_data, atlas_w, atlas_h);

        // Per-tile texture orientation (angle bits 4-6); orientation 0
        // for packages baked before the angle member existed.
        let flat_angle;
        let angle: &[u8] = match &level.angle {
            Some(a) => {
                assert_eq!(a.len(), n * n);
                a
            }
            None => {
                flat_angle = vec![0u8; n * n];
                &flat_angle
            }
        };
        let angle_tex = byte_tex("tile angles", angle, n as u32, n as u32);
        let height_tex = byte_tex("tile heights", &level.height, n as u32, n as u32);

        // Colormap (x = palette index, y = shade): the engine's shade
        // remap composed with the palette on the CPU. sRGB format so
        // sampling yields linear color. Texture texels and flat tile
        // colors both resolve through this one table, exactly like the
        // original's textured inner loop `shade_lut[shade*256 + texel]`.
        assert_eq!(level.shade_lut.len(), SHADE_LEVELS * 256);
        let mut colormap = vec![0u8; SHADE_LEVELS * 256 * 4];
        for shade in 0..SHADE_LEVELS {
            for index in 0..256 {
                let final_idx = level.shade_lut[shade * 256 + index] as usize;
                let rgb = level.palette[final_idx];
                let o = (shade * 256 + index) * 4;
                colormap[o..o + 3].copy_from_slice(&rgb);
                colormap[o + 3] = 255;
            }
        }
        let colormap_extent = wgpu::Extent3d {
            width: 256,
            height: SHADE_LEVELS as u32,
            depth_or_array_layers: 1,
        };
        let colormap_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("type/shade colormap"),
            size: colormap_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            colormap_tex.as_image_copy(),
            &colormap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: None,
            },
            colormap_extent,
        );

        self.colormap_tex = Some(colormap_tex.clone());
        self.rebuild_billboard_bind_group();

        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &type_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &shade_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        &colormap_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &tile_colors_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &atlas_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &angle_tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        &height_tex.create_view(&Default::default()),
                    ),
                },
            ],
        }));
        self.plane_texs = Some([type_tex, shade_tex, angle_tex, height_tex]);

        // Overhead map for the book screen, composed on the CPU through
        // the engine's map color path.
        let map_rgba = map_pixels(level, overlay);
        let map_extent = wgpu::Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        };
        let map_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("overhead map"),
            size: map_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            map_tex.as_image_copy(),
            &map_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n as u32 * 4),
                rows_per_image: None,
            },
            map_extent,
        );
        let map_view = map_tex.create_view(&Default::default());
        self.map_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map"),
            layout: &self.map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.map_globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&map_view),
                },
            ],
        }));
        // The in-flight minimap shares the world map texture but has its
        // own globals (corner rect, tighter zoom, round mask).
        self.minimap_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("minimap"),
            layout: &self.map_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.minimap_globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&map_view),
                },
            ],
        }));
        self.map_tex = Some(map_tex);
    }

    /// Re-upload the terrain planes + overhead map after runtime world
    /// mutation (craters, quakes, spawned entities). The level view
    /// must carry the LIVE planes; mesh and bind groups are reused —
    /// this is four 64 KB texture writes plus the map compose.
    pub fn update_terrain(&mut self, level: &LevelView, overlay: &MapOverlay) {
        let n = MAP_TILES as u32;
        let Some([type_tex, shade_tex, angle_tex, height_tex]) = &self.plane_texs else {
            return;
        };
        let write = |tex: &wgpu::Texture, bytes: &[u8]| {
            self.queue.write_texture(
                tex.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: n,
                    height: n,
                    depth_or_array_layers: 1,
                },
            );
        };
        write(type_tex, &level.tile_type);
        write(height_tex, &level.height);
        if let Some(s) = &level.shading {
            write(shade_tex, s);
        }
        if let Some(a) = &level.angle {
            write(angle_tex, a);
        }
        self.update_map(level, overlay);
    }

    /// Recompose + re-upload ONLY the overhead map texture (dots,
    /// icon stamps, the guide path, blink phases). Cheap enough to
    /// run every sim tick — the original redraws its map every frame,
    /// and the blink/marching-ants patterns need it.
    pub fn update_map(&mut self, level: &LevelView, overlay: &MapOverlay) {
        let n = MAP_TILES as u32;
        if let Some(map_tex) = &self.map_tex {
            let map_rgba = map_pixels(level, overlay);
            self.queue.write_texture(
                map_tex.as_image_copy(),
                &map_rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: n,
                    height: n,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Upload the bundle's sprite atlas + index for billboard drawing.
    pub fn load_sprites(&mut self, index: mgc_formats::bundle::SpriteIndex, atlas: &[u8]) {
        assert_eq!(
            atlas.len(),
            index.atlas_width as usize * index.atlas_height as usize
        );
        let extent = wgpu::Extent3d {
            width: index.atlas_width,
            height: index.atlas_height,
            depth_or_array_layers: 1,
        };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite atlas"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            tex.as_image_copy(),
            atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(index.atlas_width),
                rows_per_image: None,
            },
            extent,
        );
        self.sprite_tex = Some(tex);
        self.sprite_index = Some(index);
        self.rebuild_billboard_bind_group();
    }

    /// Replace the set of world sprites drawn each frame.
    /// Upload the RGBA UI atlas (app-side composited: HSPR indices
    /// resolved through the blend LUT + palette; index-0 texels carry
    /// alpha 0).
    pub fn load_ui_atlas(&mut self, width: u32, height: u32, rgba: &[u8]) {
        debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
        self.ui_atlas_size = (width.max(1), height.max(1));
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.ui_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui"),
            layout: &self.ui_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.ui_globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &tex.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }));
    }

    /// Replace this frame's UI quads (drawn last, in list order).
    pub fn set_ui_quads(&mut self, quads: Vec<UiQuad>) {
        self.ui_quads = quads;
    }

    /// Set the upright map icons (own castle/balloons). They are drawn
    /// screen-space over the active map surface — never baked into the
    /// rotated map texture — so they stay upright under rotation.
    pub fn set_map_stamps(&mut self, stamps: Vec<MapStamp>) {
        self.map_stamps = stamps;
    }

    /// Set the marching-ants guide path (player → own castle). Drawn
    /// screen-space over the active map surface, a mark every 4
    /// surface pixels (see [`project_guide_path`]); None = no path.
    pub fn set_map_path(&mut self, path: Option<MapPath>) {
        self.map_path = path;
    }

    /// The in-flight radar disc: (diameter, center_x, center_y) in
    /// pixels. The disc is anchored at the screen CORNER (0,0) so its
    /// center sits at its radius (retail DrawMinimap(0,0)) — scaled by
    /// the HUD factor (w/640) to track the sprite panels. Single source
    /// of truth for both the shader uniform and the stamp projection;
    /// they MUST agree or terrain and stamps diverge.
    fn minimap_rect(&self, w: u32, hpx: u32) -> (f32, f32, f32) {
        let hud = w as f32 / 640.0;
        let diam = (MINIMAP_DIAM * hud).min(w.min(hpx) as f32);
        // Anchored at the corner (0,0), touching both screen edges — the
        // disc center is exactly at its radius (retail DrawMinimap(0,0)).
        let c = diam * 0.5;
        (diam, c, c)
    }

    /// Project the map stamps onto one map surface as upright UI quads.
    /// `center`/`half` are the surface's screen rect (pixels): center
    /// point and half-extents. `zoom` = tiles across the shorter axis,
    /// `round` clips to the inscribed disc, `scale` = the surface's
    /// native→screen pixel factor (stamps keep their retail proportion
    /// at any window size). Mirrors the sampling transform in map.wgsl
    /// (inverted); see [`project_map_stamps`].
    #[allow(clippy::too_many_arguments)]
    fn map_stamp_quads(
        &self,
        cx: f32,
        cy: f32,
        half_x: f32,
        half_y: f32,
        px: f32,
        pz: f32,
        yaw: f32,
        zoom: f32,
        round: bool,
        aspect: f32,
        scale: f32,
    ) -> Vec<UiQuad> {
        project_map_stamps(
            &self.map_stamps,
            cx,
            cy,
            half_x,
            half_y,
            px,
            pz,
            yaw,
            zoom,
            round,
            aspect,
            scale,
        )
    }

    pub fn set_billboards(&mut self, billboards: Vec<Billboard>) {
        self.billboards = billboards;
    }

    /// Replace the monster health-bar overlay set (empty = off).
    pub fn set_health_bars(&mut self, bars: Vec<HealthBar>) {
        self.health_bars = bars;
    }

    fn rebuild_billboard_bind_group(&mut self) {
        let (Some(sprites), Some(colormap)) = (&self.sprite_tex, &self.colormap_tex) else {
            return;
        };
        self.billboard_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("billboard"),
                layout: &self.billboard_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.globals_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &sprites.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &colormap.create_view(&Default::default()),
                        ),
                    },
                ],
            }));
    }

    /// Resolve each billboard against the camera (rotation view,
    /// mirroring, wrap-nearest position) into instance data — the
    /// original's per-sprite draw dispatch (remc1 DrawSprite3D_2F170),
    /// with the yaw quantization done in engine angle units.
    fn billboard_instances(&self, cam: &CameraView) -> Vec<BillboardInstance> {
        let Some(index) = &self.sprite_index else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(self.billboards.len());
        let full = MAP_TILES as f32;
        for b in &self.billboards {
            // 16 view sectors from relative yaw, exactly the engine's
            // `(((entityYaw - camYaw) >> 3) & 0xF0) >> 4` on 11-bit
            // angles: floor(rel / 128) of 2048 steps.
            let rel = (b.yaw - cam.yaw).rem_euclid(std::f32::consts::TAU);
            let view = ((rel * (2048.0 / std::f32::consts::TAU)) as i32 >> 7).clamp(0, 15) as u16;
            let (offset, mirror) = match b.draw_type {
                17 => {
                    if view < 8 {
                        (view, false)
                    } else {
                        (15 - view, true)
                    }
                }
                18 => (view, false),
                19 => (VIEW_FOLD_5[view as usize] as u16, view >= 8),
                20 => (VIEW_FOLD_3[view as usize] as u16, view >= 8),
                // Animation modes: the entity's anim byte selects the
                // family member (DrawSprite3D :37552).
                2..=16 => (b.frame as u16, false),
                // 0/1/21 single view, and anything unknown: base.
                _ => (0, false),
            };
            let id = (b.sprite_base + offset) as usize;
            let Some(entry) = index.sprites.get(id) else {
                continue;
            };
            if entry.frames.is_empty() {
                continue; // known-corrupt source entry
            }
            // Animated entries (flags bit 0, the TMAPS FLC streams) step
            // one frame per turn in a forward loop, all in lockstep —
            // the original's per-frame driver (remc1 sub_590D0_595E0).
            let fi = if entry.flags & 1 != 0 {
                self.anim_turn as usize % entry.frames.len()
            } else {
                0
            };
            let frame = &entry.frames[fi];
            let (w, h) = (entry.width as f32, entry.height as f32);
            let world_w = b.world_h * w / h;
            // Nearest torus copy relative to the camera.
            let wrap = |p: f32, c: f32| {
                let mut d = p - c;
                if d > full / 2.0 {
                    d -= full;
                }
                if d < -full / 2.0 {
                    d += full;
                }
                c + d
            };
            out.push(BillboardInstance {
                pos: [wrap(b.x, cam.x), b.y, wrap(b.z, cam.z)],
                size: [world_w, b.world_h],
                uv_pos: [frame.x as f32, frame.y as f32],
                uv_size: [w, h],
                flags: [mirror as u32, 32],
                _pad: [0],
            });
        }
        out
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if let Target::Window { surface, config } = &mut self.target {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        }
        self.depth = create_depth(&self.device, width, height);
    }

    fn size(&self) -> (u32, u32) {
        match &self.target {
            Target::Window { config, .. } => (config.width, config.height),
            Target::Offscreen { width, height, .. } => (*width, *height),
        }
    }

    /// Render one frame.
    pub fn render(&mut self, cam: &CameraView) -> Result<(), wgpu::SurfaceError> {
        let (w, hpx) = self.size();

        // Book-screen layout (sub_20E60 case 4), native 640×480 scaled to
        // the live resolution. The live world fills the background; the
        // 382×378 map pane pastes top-left and the spellbook grid fills
        // bottom-right, leaving the world visible in the top-right corner
        // (right of the map, above the spellbook) and the bottom strip.
        // Native→screen scale for the book layout (kept distinct from the
        // camera basis's `sx/sy` sin/cos below — the collision zeroed the
        // map pane's height when yaw=0).
        let res_x = w as f32 / 640.0;
        let res_y = hpx as f32 / 480.0;
        // The world viewport = the top-right rectangle. Its LEFT edge is
        // the SPELLBOOK's left (384); its BOTTOM recedes by BOOK_GAP above
        // the spellbook top (194−2) so a 2px black gap separates them —
        // the horizontal bar of the "T" demarcation (player 2026-07-07;
        // the gap comes out of the live view, not the spellbook).
        let view_rect = (
            (BOOK_SPELL_X * res_x) as u32,
            0u32,
            w.saturating_sub((BOOK_SPELL_X * res_x) as u32),
            ((BOOK_SPELL_Y - BOOK_GAP) * res_y) as u32,
        );

        let aspect = if self.map_view {
            view_rect.2 as f32 / view_rect.3.max(1) as f32
        } else {
            w as f32 / hpx as f32
        };
        let view_proj = camera_matrix(cam, aspect);
        let sky = sky_color_linear();
        // Camera right/up for billboard expansion (matches
        // `camera_matrix`'s basis).
        let (sy, cy) = cam.yaw.sin_cos();
        let (sp, cp) = cam.pitch.sin_cos();
        let fwd = [sy * cp, sp, -cy * cp];
        let right = [cy, 0.0, sy];
        let up = [
            right[1] * fwd[2] - right[2] * fwd[1],
            right[2] * fwd[0] - right[0] * fwd[2],
            right[0] * fwd[1] - right[1] * fwd[0],
        ];
        let globals = Globals {
            view_proj,
            camera: [cam.x, cam.y, cam.z, FOG_DENSITY],
            // The fog alpha slot carries the animation clock (turns).
            fog_color: [sky[0] as f32, sky[1] as f32, sky[2] as f32, self.anim_turn],
            atlas: [
                self.atlas_cells,
                self.smooth_shading as u32,
                self.wave_mode,
                0,
            ],
            cam_right: [right[0], right[1], right[2], 0.0],
            cam_up: [up[0], up[1], up[2], 0.0],
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Billboard instances for this camera (empty when no sprites
        // are loaded).
        let instances = self.billboard_instances(cam);
        let instance_count = instances.len() as u32;
        if !instances.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(&instances);
            let need = bytes.len();
            if self.billboard_buf.is_none() || self.billboard_capacity < need {
                self.billboard_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("billboard instances"),
                    size: need.next_power_of_two() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.billboard_capacity = need.next_power_of_two();
            }
            self.queue
                .write_buffer(self.billboard_buf.as_ref().unwrap(), 0, bytes);
        }

        // Health-bar instances (wrap-nearest like billboards).
        let full = MAP_TILES as f32;
        let wrapn = |p: f32, c: f32| {
            let mut d = p - c;
            if d > full / 2.0 {
                d -= full;
            }
            if d < -full / 2.0 {
                d += full;
            }
            c + d
        };
        let bar_instances: Vec<BarInstance> = self
            .health_bars
            .iter()
            .map(|b| BarInstance {
                pos: [wrapn(b.x, cam.x), b.y, wrapn(b.z, cam.z)],
                size: [b.w, 0.09],
                frac: b.frac.clamp(0.0, 1.0),
            })
            .collect();
        let bar_count = bar_instances.len() as u32;
        if !bar_instances.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(&bar_instances);
            let need = bytes.len();
            if self.bar_buf.is_none() || self.bar_capacity < need {
                self.bar_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("bar instances"),
                    size: need.next_power_of_two() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.bar_capacity = need.next_power_of_two();
            }
            self.queue
                .write_buffer(self.bar_buf.as_ref().unwrap(), 0, bytes);
        }

        // Screen-space map decorations — upright icon stamps
        // (castle/balloon) and the marching-ants guide path — projected
        // onto whichever map surface is active and appended to the UI
        // quad stream, so they draw unrotated/evenly-spaced over the
        // rotated map. Rect math mirrors the map-globals block below.
        let mut stamp_quads: Vec<UiQuad> = Vec::new();
        {
            // (center, half-extents, zoom, round, aspect, scale) of the
            // active surface, shared by stamps and path.
            let surface = if self.map_view {
                // Same pane rect as the map-globals block, in px.
                let (pw, ph) = (BOOK_MAP_W * res_x, BOOK_MAP_H * res_y);
                let cx = (BOOK_MAP_X * res_x) + pw * 0.5;
                let cy = (BOOK_MAP_Y * res_y) + ph * 0.5;
                // Icons scale with the pane like every book element
                // (retail only ever rendered ≤640 wide; native-size
                // icons at HD read a third of their proportion).
                Some((
                    cx,
                    cy,
                    pw * 0.5,
                    ph * 0.5,
                    BOOK_MAP_ZOOM,
                    false,
                    pw / ph,
                    res_x,
                ))
            } else {
                // Same (diam, center) as the shader uniform — shared via
                // minimap_rect so terrain and stamps can't diverge. The
                // scale tracks the disc (128 native px × w/640, possibly
                // clamped on tiny windows).
                let (disc, cx, cy) = self.minimap_rect(w, hpx);
                Some((
                    cx,
                    cy,
                    disc * 0.5,
                    disc * 0.5,
                    self.minimap_zoom,
                    true,
                    1.0,
                    disc / MINIMAP_DIAM,
                ))
            };
            if let Some((cx, cy, hx, hy, zoom, round, aspect, scale)) = surface {
                stamp_quads = self.map_stamp_quads(
                    cx, cy, hx, hy, cam.x, cam.z, cam.yaw, zoom, round, aspect, scale,
                );
                if let Some(path) = &self.map_path {
                    stamp_quads.extend(project_guide_path(
                        path, cx, cy, hx, hy, cam.x, cam.z, cam.yaw, zoom, round, aspect, scale,
                    ));
                }
            }
        }

        // UI quads (screen-space overlay, both views) + the projected
        // map stamps/ants on top — written as two regions of one
        // vertex buffer (no per-frame concatenation copy).
        let ui_count = (self.ui_quads.len() + stamp_quads.len()) as u32;
        if ui_count > 0 {
            self.queue.write_buffer(
                &self.ui_globals_buf,
                0,
                bytemuck::cast_slice(&[w as f32, hpx as f32, 0.0, 0.0]),
            );
            let ui_bytes: &[u8] = bytemuck::cast_slice(&self.ui_quads);
            let stamp_bytes: &[u8] = bytemuck::cast_slice(&stamp_quads);
            let need = ui_bytes.len() + stamp_bytes.len();
            if self.ui_buf.is_none() || self.ui_capacity < need {
                self.ui_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ui quads"),
                    size: need.next_power_of_two() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.ui_capacity = need.next_power_of_two();
            }
            let buf = self.ui_buf.as_ref().unwrap();
            if !ui_bytes.is_empty() {
                self.queue.write_buffer(buf, 0, ui_bytes);
            }
            if !stamp_bytes.is_empty() {
                self.queue
                    .write_buffer(buf, ui_bytes.len() as u64, stamp_bytes);
            }
        }

        let frame = match &self.target {
            Target::Window { surface, .. } => Some(surface.get_current_texture()?),
            Target::Offscreen { .. } => None,
        };
        let color_view = match (&frame, &self.target) {
            (Some(f), _) => f.texture.create_view(&Default::default()),
            (None, Target::Offscreen { color, .. }) => color.create_view(&Default::default()),
            _ => unreachable!(),
        };

        if self.map_view {
            // The book map pane at native (0,0) 382×378, player-centered
            // and yaw-rotated, rectangular (round mask off). Placed by
            // pixel rect → NDC so it matches the stamp projection.
            let (px0, py0) = (BOOK_MAP_X * res_x, BOOK_MAP_Y * res_y);
            let (pw, ph) = (BOOK_MAP_W * res_x, BOOK_MAP_H * res_y);
            let cx_px = px0 + pw * 0.5;
            let cy_px = py0 + ph * 0.5;
            let map_globals: [f32; 12] = [
                cx_px / w as f32 * 2.0 - 1.0,   // pixel center → NDC x
                1.0 - cy_px / hpx as f32 * 2.0, // pixel center → NDC y (flip)
                pw / w as f32,                  // NDC half-width
                ph / hpx as f32,                // NDC half-height
                cam.x,
                cam.z,
                cam.yaw,
                BOOK_MAP_ZOOM,
                0.0,              // rectangular (no round mask)
                pw / ph,          // sampler aspect = pane w/h
                1.0,              // opaque (the map pane sits over the world)
                MAP_TILES as f32, // world period for the toroidal wrap
            ];
            self.queue
                .write_buffer(&self.map_globals_buf, 0, bytemuck::cast_slice(&map_globals));
        } else {
            // In-flight round minimap, corner-anchored at (0,0). Disc +
            // position scale with the HUD (w/640).
            let (disc, cx, cy) = self.minimap_rect(w, hpx);
            let hw = disc / w as f32; // NDC half-width
            let hh = disc / hpx as f32; // NDC half-height
            let minimap_globals: [f32; 12] = [
                cx / w as f32 * 2.0 - 1.0,   // pixel center → NDC x
                1.0 - cy / hpx as f32 * 2.0, // pixel center → NDC y (flip)
                hw,
                hh,
                cam.x,
                cam.z,
                cam.yaw,
                self.minimap_zoom,
                1.0,                // round mask
                1.0,                // square disc → aspect 1
                self.minimap_alpha, // HUD transparency
                MAP_TILES as f32,   // world period for the toroidal wrap
            ];
            self.queue.write_buffer(
                &self.minimap_globals_buf,
                0,
                bytemuck::cast_slice(&minimap_globals),
            );
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            // The book screen: the world viewport fills the top-right,
            // the map pane the top-left, the spellbook the bottom-right;
            // everything below (the message-log zone) is pure BLACK in
            // retail — the clear shows through with no panel fill.
            let clear = if self.map_view {
                wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: sky[0],
                    g: sky[1],
                    b: sky[2],
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let draw_world = |pass: &mut wgpu::RenderPass<'_>| {
                if let (Some(bg), Some(vb), Some(ib)) =
                    (&self.bind_group, &self.vertex_buf, &self.index_buf)
                {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    // 3x3 wrap copies; the vertex shader offsets by instance.
                    pass.draw_indexed(0..self.index_count, 0, 0..9);
                }
                if let (1.., Some(bg), Some(buf)) = (
                    instance_count,
                    &self.billboard_bind_group,
                    &self.billboard_buf,
                ) {
                    pass.set_pipeline(&self.billboard_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..6, 0..instance_count);
                }
                if let (1.., Some(buf)) = (bar_count, &self.bar_buf) {
                    pass.set_pipeline(&self.bar_pipeline);
                    pass.set_bind_group(0, &self.bar_bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..6, 0..bar_count);
                }
            };
            if self.map_view {
                // World viewport in the top-right corner: sky fill, then
                // the terrain, clipped to the rect.
                let (vx, vy, vw, vh) = view_rect;
                if vw > 0 && vh > 0 {
                    pass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                    pass.set_scissor_rect(vx, vy, vw, vh);
                    pass.set_pipeline(&self.fill_pipeline);
                    pass.draw(0..3, 0..1);
                    draw_world(&mut pass);
                    pass.set_viewport(0.0, 0.0, w as f32, hpx as f32, 0.0, 1.0);
                    pass.set_scissor_rect(0, 0, w, hpx);
                }
                // The map pane; the rest of the dark clear is the book
                // backdrop (spell list placeholder).
                if let Some(bg) = &self.map_bind_group {
                    pass.set_pipeline(&self.map_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.draw(0..6, 0..1);
                }
            } else {
                draw_world(&mut pass);
                // In-flight round minimap in the corner (round mask
                // discards outside the disc); present once a level is
                // loaded.
                if let Some(bg) = &self.minimap_bind_group {
                    pass.set_pipeline(&self.map_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.draw(0..6, 0..1);
                }
            }
            // Screen-space UI on top of either view.
            if let (1.., Some(bg), Some(buf)) = (ui_count, &self.ui_bind_group, &self.ui_buf) {
                pass.set_pipeline(&self.ui_pipeline);
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..6, 0..ui_count);
            }
        }
        self.queue.submit([encoder.finish()]);
        if let Some(frame) = frame {
            frame.present();
        }
        Ok(())
    }

    /// Read back the offscreen target as tightly-packed RGBA8 rows.
    /// Panics if the renderer targets a window.
    pub fn read_offscreen(&self) -> (u32, u32, Vec<u8>) {
        let Target::Offscreen {
            color,
            width,
            height,
        } = &self.target
        else {
            panic!("read_offscreen on a windowed renderer");
        };
        let (width, height) = (*width, *height);
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).ok();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("buffer map failed");
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        (width, height, out)
    }
}

fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: surface,
        force_fallback_adapter: false,
    }))
    .ok_or(RenderError::NoAdapter)?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("mgcarpet"),
            ..Default::default()
        },
        None,
    ))
    .map_err(|e| RenderError::Device(e.to_string()))?;
    Ok((adapter, device, queue))
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Column-major view-projection matrix. Yaw 0 faces -Z, positive pitch
/// looks up; right-handed, Y-up, depth 0..1.
fn camera_matrix(cam: &CameraView, aspect: f32) -> [[f32; 4]; 4] {
    let (sy, cy) = cam.yaw.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    let fwd = [sy * cp, sp, -cy * cp];
    let right = [cy, 0.0, sy];
    let up = [
        right[1] * fwd[2] - right[2] * fwd[1],
        right[2] * fwd[0] - right[0] * fwd[2],
        right[0] * fwd[1] - right[1] * fwd[0],
    ];
    let eye = [cam.x, cam.y, cam.z];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    // View matrix: camera basis rows, look direction mapped to -Z.
    let view = [
        [right[0], up[0], -fwd[0], 0.0],
        [right[1], up[1], -fwd[1], 0.0],
        [right[2], up[2], -fwd[2], 0.0],
        [-dot(right, eye), -dot(up, eye), dot(fwd, eye), 1.0],
    ];

    // Perspective, near 0.05 tiles, far 600 (a 256-tile world plus fog
    // headroom), depth 0..1.
    let (near, far) = (0.05_f32, 600.0_f32);
    let f = 1.0 / (cam.fov_y * 0.5).tan();
    let proj = [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ];

    // proj * view, both column-major.
    let mut out = [[0.0f32; 4]; 4];
    for (c, out_col) in out.iter_mut().enumerate() {
        for (r, out_cell) in out_col.iter_mut().enumerate() {
            *out_cell = (0..4).map(|k| proj[k][r] * view[c][k]).sum();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp_at(x: f32, z: f32) -> MapStamp {
        MapStamp {
            x,
            z,
            w: 16,
            h: 15,
            uv: [0.0, 0.0, 16.0, 15.0],
            anchor: [0.0, 1.0],
        }
    }

    #[test]
    fn book_map_stamp_survives_a_full_yaw_sweep() {
        // Any delta within the pane's inscribed 128-tile disc is
        // visible at EVERY heading (|R·d| ≤ |d| ≤ both half-spans), so
        // a (+90,+90) stamp must never vanish across a full rotation —
        // the rotated-space cull can't lose it. (Deltas OUTSIDE the
        // disc can legitimately leave the pane at diagonal headings —
        // a 256-tile span rotated 45° misses some wrapped images; the
        // map texture hides those tiles too, and the projection
        // matches the shader image-for-image by construction.)
        let stamps = [stamp_at(140.0, 218.0)]; // (+90,+90) from (50,128)
        let (pw, ph) = (382.0, 416.0);
        for i in 0..=90 {
            let yaw = i as f32 * std::f32::consts::TAU / 90.0;
            let quads = project_map_stamps(
                &stamps,
                pw * 0.5,
                ph * 0.5,
                pw * 0.5,
                ph * 0.5,
                50.0,
                128.0,
                yaw,
                256.0,
                false,
                pw / ph,
                1.0,
            );
            assert!(
                !quads.is_empty(),
                "stamp vanished at yaw {yaw:.3} (step {i})"
            );
        }
    }

    #[test]
    fn edge_stamps_draw_their_wrap_duplicate() {
        // The pane's y-span (256/aspect ≈ 279 tiles) exceeds the world
        // period, so tiles near the top/bottom edge appear TWICE — the
        // shader's fract() shows both, and the projection must emit
        // both quads (the old shortest-wrap code drew only one, so the
        // second copy's icon was missing at the opposite edge).
        let stamps = [stamp_at(50.0, 128.0 + 139.0)]; // near the +y limit
        let (pw, ph) = (382.0, 416.0);
        let quads = project_map_stamps(
            &stamps,
            pw * 0.5,
            ph * 0.5,
            pw * 0.5,
            ph * 0.5,
            50.0,
            128.0,
            0.0,
            256.0,
            false,
            pw / ph,
            1.0,
        );
        assert_eq!(quads.len(), 2, "both wrap images of an edge stamp draw");
    }

    #[test]
    fn stamps_scale_and_clip_to_the_surface() {
        // 2× scale doubles the rect; a stamp whose anchor sits just
        // inside the pane edge is clipped to the bounds with its uv
        // trimmed proportionally (never dropped, never bleeding).
        let stamps = [stamp_at(10.0, 128.0)];
        let (pw, ph) = (382.0, 416.0);
        let run = |scale: f32| {
            project_map_stamps(
                &stamps,
                pw * 0.5,
                ph * 0.5,
                pw * 0.5,
                ph * 0.5,
                10.0,
                128.0,
                0.0,
                256.0,
                false,
                pw / ph,
                scale,
            )
        };
        let q1 = run(1.0);
        let q2 = run(2.0);
        assert_eq!(q1.len(), 1);
        assert_eq!(q2.len(), 1);
        assert!(
            (q2[0].rect[2] - q1[0].rect[2] * 2.0).abs() < 1e-3,
            "rect scales"
        );

        // Anchor just inside the pane's left edge: bottom-left-anchored
        // sprite extends UP from the point; the top may clip at y=0
        // when near the top edge. Force a corner case: player centered,
        // stamp at the pane center → no clipping; stamp image near the
        // pane's top-left corner → the rect is clipped to bounds.
        let corner = [stamp_at(10.0 - 190.9, 128.0 - 276.0)]; // near pane top-left
        let q = project_map_stamps(
            &corner,
            pw * 0.5,
            ph * 0.5,
            pw * 0.5,
            ph * 0.5,
            10.0,
            128.0,
            0.0,
            256.0,
            false,
            pw / ph,
            1.0,
        );
        for quad in &q {
            assert!(
                quad.rect[0] >= 0.0 && quad.rect[1] >= 0.0,
                "clipped to bounds"
            );
            assert!(quad.rect[0] + quad.rect[2] <= pw + 1e-3);
            assert!(quad.rect[1] + quad.rect[3] <= ph + 1e-3);
            assert!(quad.uv[2] > 0.0, "uv width stays positive (textured mode)");
        }
    }
}
