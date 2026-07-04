// Terrain pass: palette-indexed terrain textures (or flat tile colors),
// per-vertex hillshade, distance fog toward the horizon color.
//
// Color path stays index-based to the end (README design): the fragment
// shader resolves an 8-bit palette index — an atlas texel when the
// level has a terrain-texture atlas, else the tile's flat color from
// the tile-colors LUT — then feeds it through the engine's shade remap
// composed with the palette (t_colormap, sRGB texture):
//   rgb = palette[shade_lut[shade][index]]
// exactly the original's textured-terrain inner loop (remc2
// GameRenderOriginal "mode 7": shade_lut[shade*256 + texel]).

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz = camera position (tile units), w = fog density
    camera: vec4<f32>,
    // rgb = fog/sky color (linear), a unused
    fog_color: vec4<f32>,
    // x = atlas cell count (0 = untextured),
    // y = smooth shading (1 = interpolate the per-tile shade level
    //     across tile centers instead of the original's per-tile snap),
    // z/w reserved
    atlas: vec4<u32>,
};

// Atlas geometry: 256 px wide, 32x32 cells, 8 per row (BLK*-1.DAT).
const ATLAS_CELL: i32 = 32;
const ATLAS_CELLS_PER_ROW: i32 = 8;

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var t_type: texture_2d<u32>;
// Per-tile light level (the generator's shading array), same layout.
@group(0) @binding(2) var t_shade: texture_2d<u32>;
// Colormap: x = palette index (texel or flat color), y = shade level;
// palette[shade_lut[shade][x]] composed on the CPU.
@group(0) @binding(3) var t_colormap: texture_2d<f32>;
// Terrain type -> flat base palette index (tile-colors.bin), 256x1.
@group(0) @binding(4) var t_tile_colors: texture_2d<u32>;
// Terrain-texture atlas, 8-bit palette indices; 1x1 dummy when absent.
@group(0) @binding(5) var t_atlas: texture_2d<u32>;
// Per-tile angle/flags byte; bits 4-6 = texture UV orientation.
@group(0) @binding(6) var t_angle: texture_2d<u32>;

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

// Shade level of a tile, wrapped to the torus, clamped to the LUT.
fn shade_at(t: vec2<i32>) -> f32 {
    let wrapped = vec2<i32>((t.x % 256 + 256) % 256, (t.y % 256 + 256) % 256);
    return f32(min(textureLoad(t_shade, wrapped, 0).r, 63u));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Tile index from world position, wrapped to the 256x256 torus.
    let tile = vec2<i32>(
        (i32(floor(in.world.x)) % 256 + 256) % 256,
        (i32(floor(in.world.z)) % 256 + 256) % 256,
    );
    let ty = i32(textureLoad(t_type, tile, 0).r);

    // Palette index: atlas texel (terrain type = atlas cell, nearest
    // sampling like the original rasterizer) or the flat tile color.
    var index: i32;
    if globals.atlas.x > 0u && ty < i32(globals.atlas.x) {
        let cell = vec2<i32>(ty % ATLAS_CELLS_PER_ROW, ty / ATLAS_CELLS_PER_ROW);
        // UV orientation from the angle byte (engine UVTable_D4350,
        // world-space rows): bit 4 flips x, bit 5 flips y, bit 6 swaps
        // the axes. Transition tiles (shorelines) depend on this.
        let orient = (textureLoad(t_angle, tile, 0).r >> 4u) & 7u;
        var st = fract(in.world.xz);
        if (orient & 1u) != 0u {
            st.x = 1.0 - st.x;
        }
        if (orient & 2u) != 0u {
            st.y = 1.0 - st.y;
        }
        if (orient & 4u) != 0u {
            st = st.yx;
        }
        let within = vec2<i32>(
            min(i32(st.x * f32(ATLAS_CELL)), ATLAS_CELL - 1),
            min(i32(st.y * f32(ATLAS_CELL)), ATLAS_CELL - 1),
        );
        index = i32(textureLoad(t_atlas, cell * ATLAS_CELL + within, 0).r);
    } else {
        index = i32(textureLoad(t_tile_colors, vec2<i32>(ty, 0), 0).r);
    }

    var base: vec3<f32>;
    if globals.atlas.y == 1u {
        // Smooth shading (opt-in enhancement): bilinear shade over the
        // four nearest tile centers, then a linear blend between the
        // two straddling shade-LUT rows. Colors still come only from
        // LUT rows — the palette pipeline stays intact, the light
        // gradient just stops snapping at tile edges.
        let p = in.world.xz - vec2<f32>(0.5, 0.5);
        let t0 = vec2<i32>(i32(floor(p.x)), i32(floor(p.y)));
        let f = fract(p);
        let s = mix(
            mix(shade_at(t0), shade_at(t0 + vec2<i32>(1, 0)), f.x),
            mix(shade_at(t0 + vec2<i32>(0, 1)), shade_at(t0 + vec2<i32>(1, 1)), f.x),
            f.y,
        );
        let s0 = i32(floor(s));
        let s1 = min(s0 + 1, 63);
        base = mix(
            textureLoad(t_colormap, vec2<i32>(index, s0), 0).rgb,
            textureLoad(t_colormap, vec2<i32>(index, s1), 0).rgb,
            fract(s),
        );
    } else {
        // Original look: one shade level per tile.
        let shade = i32(shade_at(tile));
        base = textureLoad(t_colormap, vec2<i32>(index, shade), 0).rgb;
    }

    // `light` is 1.0 when the authentic shading array drives the look;
    // it carries a synthetic hillshade only for packages without one.
    let lit = base * in.light;

    let dist = distance(in.world, globals.camera.xyz);
    let fog = 1.0 - exp(-dist * globals.camera.w);
    let rgb = mix(lit, globals.fog_color.rgb, fog);
    return vec4<f32>(rgb, 1.0);
}
