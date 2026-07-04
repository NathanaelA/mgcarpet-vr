// Terrain pass: palette-indexed tile colors, per-vertex hillshade,
// distance fog toward the horizon color.
//
// Color path stays index-based to the end (README design): the fragment
// shader looks the tile's terrain-type byte up in a 256-entry color LUT
// (palette[tile_colors[type]] precombined on the CPU, sRGB texture) —
// no per-vertex colors, no texture filtering across type boundaries.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz = camera position (tile units), w = fog density
    camera: vec4<f32>,
    // rgb = fog/sky color (linear), a unused
    fog_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var t_type: texture_2d<u32>;
// Per-tile light level (the generator's shading array), same layout.
@group(0) @binding(2) var t_shade: texture_2d<u32>;
// Colormap: x = terrain type, y = shade level;
// palette[shade_lut[shade][tile_colors[type]]] composed on the CPU.
@group(0) @binding(3) var t_colormap: texture_2d<f32>;

struct VsIn {
    @builtin(instance_index) instance: u32,
    @location(0) pos: vec3<f32>,
    @location(1) light: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) light: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // The world is a 256x256 torus: draw a 3x3 grid of copies so the
    // horizon is seamless whichever way the camera flies. The fragment
    // tile lookup wraps by modulo, so copies shade identically.
    let wrap = vec3<f32>(
        (f32(in.instance % 3u) - 1.0) * 256.0,
        0.0,
        (f32(in.instance / 3u) - 1.0) * 256.0,
    );
    var out: VsOut;
    let pos = in.pos + wrap;
    out.clip = globals.view_proj * vec4<f32>(pos, 1.0);
    out.world = pos;
    out.light = in.light;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Tile index from world position, wrapped to the 256x256 torus.
    let tile = vec2<i32>(
        (i32(floor(in.world.x)) % 256 + 256) % 256,
        (i32(floor(in.world.z)) % 256 + 256) % 256,
    );
    let ty = i32(textureLoad(t_type, tile, 0).r);
    let shade = min(i32(textureLoad(t_shade, tile, 0).r), 63);
    let base = textureLoad(t_colormap, vec2<i32>(ty, shade), 0).rgb;

    // `light` is 1.0 when the authentic shading array drives the look;
    // it carries a synthetic hillshade only for packages without one.
    let lit = base * in.light;

    let dist = distance(in.world, globals.camera.xyz);
    let fog = 1.0 - exp(-dist * globals.camera.w);
    let rgb = mix(lit, globals.fog_color.rgb, fog);
    return vec4<f32>(rgb, 1.0);
}
