// Fullscreen map pass (the original's Enter screen): the flat-color
// overhead map — one texel per tile, colors precomputed on the CPU via
// the engine's map path (see map_pixels) — drawn as a square quad, plus
// a player marker. The rest of the screen is left to the clear color
// (reserved for the spell-list half of the book screen).

struct MapGlobals {
    // xy = quad center in NDC, zw = quad half-extents in NDC
    rect: vec4<f32>,
    // xy = player position in tile coordinates, zw unused
    player: vec4<f32>,
};

@group(0) @binding(0) var<uniform> mg: MapGlobals;
@group(0) @binding(1) var t_map: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
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
    // Map row 0 (world z = 0) at the top of the quad.
    out.uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let pos = in.uv * 256.0;
    let tile = clamp(vec2<i32>(pos), vec2<i32>(0), vec2<i32>(255));
    var rgb = textureLoad(t_map, tile, 0).rgb;
    // Player marker: a small white diamond, readable at any map scale.
    let d = abs(pos - mg.player.xy);
    if d.x + d.y < 2.5 {
        rgb = vec3<f32>(1.0, 1.0, 1.0);
    }
    return vec4<f32>(rgb, 1.0);
}
