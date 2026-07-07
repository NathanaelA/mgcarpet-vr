// The overhead map — both the fullscreen book screen (Enter view) and
// the in-flight round minimap. Faithful port of the original's
// DrawMinimap_49300_49640 (remc1 sub_main.cpp:57491): the map is
// PLAYER-CENTERED and YAW-ROTATED — the player sits dead center and the
// world scrolls + spins under it (not a static axis-aligned grid). The
// world map texture is composed on the CPU through the engine's color
// path (see map_pixels); here we sample it under the rotated,
// player-centered affine so entity dots / stamps / the guide path — all
// baked into the world texture — rotate together with the terrain.

struct MapGlobals {
    // xy = quad center in NDC, zw = quad half-extents in NDC
    rect: vec4<f32>,
    // xy = player position in tile coordinates (the sample center),
    // z = heading in radians (yaw), w = zoom (tiles across the quad's
    // shorter axis; smaller = more zoomed in)
    player: vec4<f32>,
    // x = round mask (1 = circular disc for the HUD minimap, 0 = the
    // rectangular book map), y = aspect (quad width / height in pixels),
    // z = output alpha (HUD transparency; 1 = opaque), w unused
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

    // Toroidal wrap into the 256-tile world, nearest-texel fetch.
    let tile = vec2<i32>(
        i32(fract(world.x / 256.0 + 1.0) * 256.0) & 255,
        i32(fract(world.y / 256.0 + 1.0) * 256.0) & 255,
    );
    var rgb = textureLoad(t_map, tile, 0).rgb;

    // Player marker: a small white square dead center (the original's
    // fixed centered marker).
    let center = abs(in.uv);
    if center.x < 0.015 && center.y < 0.015 {
        rgb = vec3<f32>(1.0, 1.0, 1.0);
    }

    // Output alpha carries the HUD transparency (radar follows the same
    // toggle as the panels); the book map passes 1.
    return vec4<f32>(rgb, mg.mode.z);
}
