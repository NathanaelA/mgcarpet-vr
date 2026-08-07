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

struct MapGlobals {
    // xy = quad center in NDC, zw = quad half-extents in NDC
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
};

@group(0) @binding(0) var<uniform> mg: MapGlobals;
@group(0) @binding(1) var t_map: texture_2d<f32>;

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
    out.clip = vec4<f32>(mg.rect.xy + c * mg.rect.zw, 0.0, 1.0);
    out.uv = c;
    return out;
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

    // Toroidal wrap into the world period (mode.w = MAP_TILES),
    // nearest-texel fetch. The % guards the fract·tiles == tiles
    // rounding edge (the old & 255 mask, period-agnostic).
    let tiles = mg.mode.w;
    let tile = vec2<i32>(
        i32(fract(world.x / tiles) * tiles) % i32(tiles),
        i32(fract(world.y / tiles) * tiles) % i32(tiles),
    );
    var rgb = textureLoad(t_map, tile, 0).rgb;

    // Player marker (sub_48710 epilogue :57449-69): a four-arm CROSS
    // at the pane center, arm length = pane_width/12 (= 1/6 of the
    // centered half-width; the vertical arms match in PIXELS, hence
    // aspect/6 in uv.y), one surface pixel thick (retail keeps 1px at
    // both its 320 and 640 surfaces — the rule is one pixel of the
    // output surface). Retail fades each step through the fog ramp
    // 0x2C00→0x2400; a linear WHITE-mix approximates that until the
    // LUT bake. POLARITY CHECK OWED: the locked-spell wash proved fog
    // row 0x30 DARKENS (player 2026-07-08), so rows 0x24-0x2C may
    // darken too — if retail's cross reads dark on the map, flip the
    // mix target to black.
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
