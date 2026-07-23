// The overhead map — both the fullscreen book screen (Enter view) and
// the in-flight round minimap. Faithful port of the original's
// DrawMinimap_49300_49640 (remc1 sub_main.cpp:57491): the map is
// PLAYER-CENTERED and YAW-ROTATED — the player sits dead center and the
// world scrolls + spins under it (not a static axis-aligned grid). The
// world map texture is composed on the CPU through the engine's color
// path (see map_pixels); here we sample it under the rotated,
// player-centered affine so the baked entity dots rotate together with
// the terrain. (Icon stamps and the guide path draw screen-space over
// this pass — upright and evenly spaced — see project_map_stamps /
// project_guide_path in lib.rs.)
//
// In VR the same quad can be pinned to the world-space HUD panel so the
// minimap background sits with the rest of the HUD instead of being
// glued to the per-eye near plane.

struct MapGlobals {
    // Screen-space mode: xy = quad center in NDC, zw = quad half-extents
    // in NDC.
    // World-space mode: xy = pixel offset from the screen centre to the
    // quad centre, zw = pixel half-extents.
    rect: vec4<f32>,
    // xy = player position in tile coordinates (the sample center),
    // z = heading in radians (yaw), w = zoom (tiles across the quad's
    // shorter axis; smaller = more zoomed in)
    player: vec4<f32>,
    // x = round mask (1 = circular disc for the HUD minimap, 0 = the
    // rectangular book map), y = aspect (quad width / height in pixels),
    // z = output alpha (HUD transparency; 1 = opaque), w = world period
    // in tiles (MAP_TILES; the toroidal wrap + texture size)
    mode: vec4<f32>,
    // x = Beyond-Sight ripple strength (0 = off), y = its animation
    // clock in seconds
    fx: vec4<f32>,
    // Per-eye view-projection matrix (used in world-space mode).
    view_proj: mat4x4<f32>,
    // World-space HUD panel basis (used in world-space mode).
    panel_origin: vec3<f32>,
    panel_scale: f32,
    panel_right: vec3<f32>,
    panel_mode: u32,
    panel_up: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> mg: MapGlobals;
@group(0) @binding(1) var t_map: texture_2d<f32>;
// The Beyond-Sight area field: one texel per tile, r = trap density,
// b = effector density (lib.rs sight_field_pixels), filtered + wrapped.
// (The channel is only a slot: the effector TINT is SIGHT_EFFECT.)
@group(0) @binding(2) var t_sight: texture_2d<f32>;
@group(0) @binding(3) var s_sight: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Centered quad coordinates in [-1,1]: (0,0) is the player.
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vi];
    var out: VsOut;

    if mg.panel_mode == 1u {
        // WORLD-SPACE HUD PANEL (VR): the rect holds the pixel offset
        // from the screen centre and the pixel half-extents.
        let px = mg.rect.xy + c * mg.rect.zw;
        let world = mg.panel_origin
                  + mg.panel_right * (px.x * mg.panel_scale)
                  + mg.panel_up    * (px.y * mg.panel_scale);
        out.clip = mg.view_proj * vec4<f32>(world, 1.0);
    } else {
        // SCREEN-SPACE: the rect is NDC centre + half-extents.
        out.clip = vec4<f32>(mg.rect.xy + c * mg.rect.zw, 0.0, 1.0);
    }

    out.uv = c;
    return out;
}

// ---- the Beyond-Sight ripple (`map_beyond_sight_areas`, a deviation:
// neither original reveals trigger areas) ------------------------------
//
// A trigger area shows as DISTURBED MAP rather than as a shape: inside
// it the map is looked up through a rippling surface, like the bed of
// a pond seen through its waves — the terrain texels swim up to a tile
// and more, a faint glint rides the swell (which is what carries the
// effect over flat-coloured ground), and only a trace of tint says
// which kind it is: red over traps, cyan over every other effect
// (cyan, not blue: it has to read against the sea). The area's own
// outline is warped by slow noise, so the edge swims and reshapes too.
// All in world tile space, so both map surfaces show the same thing
// whatever their zoom.
const SIGHT_RIPPLE: f32 = 1.7;   // peak texel displacement, tiles
const SIGHT_GLINT: f32 = 0.20;   // peak brightness swing on the swell
const SIGHT_TINT: f32 = 0.17;    // the tint's share at full density
const SIGHT_EDGE: f32 = 2.6;     // outline swim, tiles peak to peak
const SIGHT_TRAP: vec3<f32> = vec3<f32>(0.90, 0.07, 0.05);
const SIGHT_EFFECT: vec3<f32> = vec3<f32>(0.04, 0.80, 0.66);

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q = q + dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.5 * vnoise(p) + 0.3 * vnoise(p * 2.03 + 17.1) + 0.2 * vnoise(p * 4.11 + 5.7);
}

// The nearest map texel under a world position: toroidal wrap into the
// world period (mode.w = MAP_TILES). The % guards the fract·tiles ==
// tiles rounding edge (the old & 255 mask, period-agnostic).
fn map_tile(world: vec2<f32>) -> vec2<i32> {
    let tiles = mg.mode.w;
    return vec2<i32>(
        i32(fract(world.x / tiles) * tiles) % i32(tiles),
        i32(fract(world.y / tiles) * tiles) % i32(tiles),
    );
}

// The water surface: two sheets of smooth noise sliding across each
// other, the second turned off the tile axes. xy = the refraction
// offset, z = the swell height, all within ±1 (and mostly well inside
// it). NOISE, not wave trains: crossing sine trains interfere into a
// fixed lattice, which read as a moiré over the map's own dither.
fn sight_waves(p: vec2<f32>, t: f32) -> vec3<f32> {
    let turned = vec2<f32>(p.x * 0.80 - p.y * 0.60, p.x * 0.60 + p.y * 0.80);
    let a = p * 0.42 + vec2<f32>(t * 0.55, t * 0.31);
    let b = turned * 0.71 + vec2<f32>(-t * 0.47, t * 0.38);
    let x = vnoise(a) + vnoise(b + 19.3) - 1.0;
    let y = vnoise(a + 7.7) + vnoise(b + 41.9) - 1.0;
    return vec3<f32>(x, y, (x + y) * 0.5);
}

fn sight_ripple(rgb: vec3<f32>, world: vec2<f32>) -> vec3<f32> {
    let t = mg.fx.y;
    // The area field, read through a slow noise warp: the outline
    // swims and reshapes instead of standing as an edge.
    let warp = vec2<f32>(
        fbm(world * 0.13 + vec2<f32>(t * 0.07, -t * 0.05)),
        fbm(world * 0.13 + vec2<f32>(31.7 - t * 0.06, 12.3 + t * 0.08)),
    ) - 0.5;
    let field = textureSampleLevel(t_sight, s_sight, (world + warp * SIGHT_EDGE) / mg.mode.w, 0.0);
    let trap = field.r;
    let effect = field.b;
    let depth = clamp(max(trap, effect), 0.0, 1.0) * mg.fx.x;
    if depth <= 0.0 {
        return rgb;
    }
    // The water surface: the map under it is fetched displaced. A
    // displaced fetch that lands on a baked entity dot (alpha 0) keeps
    // the undisturbed texel — markers neither smear nor swim.
    let waves = sight_waves(world, t);
    let moved = textureLoad(t_map, map_tile(world + waves.xy * (SIGHT_RIPPLE * depth)), 0);
    var out = select(rgb, moved.rgb, moved.a > 0.5);
    out = out * (1.0 + SIGHT_GLINT * waves.z * depth);
    let tint = (SIGHT_TRAP * trap + SIGHT_EFFECT * effect) / max(trap + effect, 0.0001);
    return mix(out, tint, depth * SIGHT_TINT * (0.75 + 0.25 * waves.z));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Centered coordinates; correct the shorter axis so the round mask
    // is a true circle and the zoom is isotropic. mg.mode.y = w/h aspect.
    let aspect = mg.mode.y;
    // Derivative taken up front: fwidth needs uniform control flow, so
    // it must precede the round-mask discard.
    let pxw = fwidth(in.uv);
    var p = in.uv;
    if aspect >= 1.0 {
        p.x = p.x * aspect;    // wider than tall: stretch x span
    } else {
        p.y = p.y / aspect;    // taller than wide: stretch y span
    }

    // Round mask: discard outside the unit disc (HUD minimap).
    let r = length(p);
    if mg.mode.x > 0.5 && r > 1.0 {
        discard;
    }

    // Rotate the centered offset by the player heading and scale to
    // tiles. yaw 0 = north (-Z / up on the map), matching the sim's
    // convention; +yaw turns the world clockwise beneath the player.
    let half = mg.player.w * 0.5;          // tiles from center to edge
    let s = sin(mg.player.z);
    let cth = cos(mg.player.z);
    // Screen-up (-y) should map to "ahead" (world -Z rotated by yaw).
    let off = vec2<f32>(p.x * half, p.y * half);
    let world = vec2<f32>(
        mg.player.x + off.x * cth + off.y * s,
        mg.player.y + off.x * s - off.y * cth,
    );

    // Nearest-texel fetch under the toroidal wrap.
    let texel = textureLoad(t_map, map_tile(world), 0);
    var rgb = texel.rgb;
    // The Beyond-Sight ripple disturbs the terrain UNDER the baked
    // entity dots (alpha 0 marks a dot texel — the pass never outputs
    // it).
    if mg.fx.x > 0.0 && texel.a > 0.5 {
        rgb = sight_ripple(rgb, world);
    }

    // Player marker (sub_48710 epilogue :57449-69): a four-arm CROSS
    // at the pane center, arm length = pane_width/12 (= 1/6 of the
    // centered half-width; the vertical arms match in PIXELS, hence
    // aspect/6 in uv.y), one surface pixel thick (retail keeps 1px at
    // both its 320 and 640 surfaces — the rule is one pixel of the
    // output surface). Retail fades each step through the fog ramp
    // 0x2C00→0x2400; a linear WHITE-mix approximates that until the
    // LUT bake. POLARITY CHECK OWED: the locked-spell wash proved fog
    // row 0x30 DARKENS (player 2026-07-08), so rows 0x24-0x2C may
    // darken too — if retail's player marker reads dark on the map, flip
    // the mix target to black.
    let cuv = abs(in.uv);
    let arm_x = 1.0 / 6.0;      // pane_w/12 px over a pane_w/2 half-span
    let arm_y = aspect / 6.0;   // the same PIXEL length in uv.y units
    let on_h = cuv.y < pxw.y && cuv.x < arm_x;
    let on_v = cuv.x < pxw.x && cuv.y < arm_y;
    if on_h || on_v {
        // Distance along the arm: 0 at center = full bright, 1 at the
        // tip = no lift.
        let d = select(cuv.y / arm_y, cuv.x / arm_x, on_h);
        rgb = mix(rgb, vec3<f32>(1.0), (1.0 - d) * 0.85);
    }

    // Output alpha carries the HUD transparency (radar follows the same
    // toggle as the panels); the book map passes 1.
    return vec4<f32>(rgb, mg.mode.z);
}

// The extent fog (opt-in DEVIATION, `map_extent_fog`): the world is
// toroidal, so the player-centered pane repeats every entity beyond
// half a period from the player — retail simply shows the duplicates.
// This pass alpha-blends black over the whole pane, clear inside the
// TRUE-extent rectangle (axis-aligned in world space, so it rotates
// with heading here) and saturating EXTENT_FADE tiles beyond it. It
// draws AFTER the map, dots, stamps and guide overlays (player ruling:
// the fog is the topmost map layer) and before the app's own UI.
const EXTENT_FADE: f32 = 16.0;

@fragment
fn fs_fog(in: VsOut) -> @location(0) vec4<f32> {
    // The same aspect correction + heading rotation as fs_main, up to
    // the world-space delta — but NOT the toroidal wrap, which is
    // exactly what the fog exists to mark.
    let aspect = mg.mode.y;
    var p = in.uv;
    if aspect >= 1.0 {
        p.x = p.x * aspect;
    } else {
        p.y = p.y / aspect;
    }
    let half = mg.player.w * 0.5;
    let s = sin(mg.player.z);
    let cth = cos(mg.player.z);
    let off = vec2<f32>(p.x * half, p.y * half);
    let d = vec2<f32>(off.x * cth + off.y * s, off.x * s - off.y * cth);
    // Chebyshev distance against half the world period: the extent
    // rect's edge, per axis.
    let extent = mg.mode.w * 0.5;
    let m = max(abs(d.x), abs(d.y));
    let fog = smoothstep(extent, extent + EXTENT_FADE, m);
    return vec4<f32>(0.0, 0.0, 0.0, fog);
}
