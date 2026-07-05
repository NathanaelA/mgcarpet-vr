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
    // rgb = fog/sky color (linear), a = water animation turn (the
    // game's per-tick counter, fractional for render interpolation)
    fog_color: vec4<f32>,
    // x = atlas cell count (0 = untextured),
    // y = smooth shading (1 = interpolate the per-tile shade level
    //     across tile centers instead of the original's per-tile snap),
    // z = water wave rule (0 = off, 1 = MC1, 2 = MC2), w reserved
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
// Height bytes (1 = 1/8 tile), sampled per grid corner in the vertex
// stage — heights live here (not in the vertex buffer) so runtime
// terrain mutation (craters, quakes) is a texture update.
@group(0) @binding(7) var t_height: texture_2d<u32>;

struct VsIn {
    @builtin(instance_index) instance: u32,
    @location(0) pos: vec3<f32>,
    @location(1) light: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) light: f32,
    // Water-shimmer shade offset in LUT rows, interpolated across the
    // triangle exactly like the original's per-corner pnt5_32.
    @location(2) shade_wave: f32,
};

const TAU: f32 = 6.283185307179586;

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
    var pos = in.pos + wrap;
    // Altitude from the height plane (the buffer carries y = 0).
    let hg = vec2<i32>(
        (i32(in.pos.x) % 256 + 256) % 256,
        (i32(in.pos.z) % 256 + 256) % 256,
    );
    pos.y = f32(textureLoad(t_height, hg, 0).r) * 0.125;
    out.shade_wave = 0.0;

    // Water surface animation: the original's per-grid-corner sine
    // product (remc1 sub_main.cpp:33955, remc2 GameRenderOriginal:1054):
    //   sinprod = (sin[(y<<7 + turn<<S) & 0x7FF] >> 8)
    //           * (sin[(x<<7 + turn<<S) & 0x7FF] >> 8)
    // on the 2048-entry 16.16 sine table — i.e. amplitude 65536,
    // wavelength 16 tiles, phase advancing turn<<S of 2048 per tick
    // (S = 6 for MC1, 5 for MC2). Gating is per VERTEX cell, so shared
    // corners displace consistently across tiles. The wave repeats
    // every 256 tiles, so the 3x3 torus copies stay seamless.
    if globals.atlas.z != 0u {
        let g = vec2<i32>(
            (i32(in.pos.x) % 256 + 256) % 256,
            (i32(in.pos.z) % 256 + 256) % 256,
        );
        let periods_per_turn = select(1.0 / 64.0, 1.0 / 32.0, globals.atlas.z == 1u);
        let phase = globals.fog_color.a * periods_per_turn;
        let sinprod = sin(TAU * (f32(g.x) / 16.0 + phase))
            * sin(TAU * (f32(g.y) / 16.0 + phase));
        if globals.atlas.z == 1u {
            // MC1: deep-water corners only (angle bit 3, the
            // generator's open-sea flag): +-1/4 tile swell, +-8 shade
            // rows of shimmer (alt -= sinprod >> 10 in 1/256-tile alt
            // units; pnt5 += 8 * sinprod in 8.16 shade).
            if (textureLoad(t_angle, g, 0).r & 8u) != 0u {
                pos.y -= sinprod * 0.25;
                out.shade_wave = sinprod * 8.0;
            }
        } else {
            // MC2: every water corner (terrain type 0) gets a gentle
            // +-1/32 tile ripple (alt -= sinprod >> 13); the shimmer is
            // skipped where the corner's shade level is 56 or darker.
            if textureLoad(t_type, g, 0).r == 0u {
                pos.y -= sinprod * (1.0 / 32.0);
                if textureLoad(t_shade, g, 0).r < 56u {
                    out.shade_wave = sinprod * 8.0;
                }
            }
        }
    }

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
        let s = clamp(
            mix(
                mix(shade_at(t0), shade_at(t0 + vec2<i32>(1, 0)), f.x),
                mix(shade_at(t0 + vec2<i32>(0, 1)), shade_at(t0 + vec2<i32>(1, 1)), f.x),
                f.y,
            ) + in.shade_wave,
            0.0,
            63.0,
        );
        let s0 = i32(floor(s));
        let s1 = min(s0 + 1, 63);
        base = mix(
            textureLoad(t_colormap, vec2<i32>(index, s0), 0).rgb,
            textureLoad(t_colormap, vec2<i32>(index, s1), 0).rgb,
            fract(s),
        );
    } else {
        // Original look: one shade level per tile, plus the water
        // shimmer. The original rounds: pnt5 carries (shade<<8 + 128)
        // <<8 + 8*sinprod and the rasterizer truncates the top byte.
        let shade = clamp(i32(round(shade_at(tile) + in.shade_wave)), 0, 63);
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
