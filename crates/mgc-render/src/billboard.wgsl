// Billboard (world sprite) pass: screen-aligned quads anchored at the
// entity's feet, textured from the bundle's 8bpp sprite atlas.
//
// Same palette-index color path as terrain: the fragment resolves an
// atlas texel (8-bit palette index; 0 = transparent, exactly the
// original blitter's per-pixel skip) through the colormap
// (palette[shade_lut[shade][index]]) and applies the shared distance
// fog. Pixels stay chunky: integer texel loads, no filtering — the
// billboard is the original sprite scaled, like the engine's affine
// rasterizer.

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    fog_color: vec4<f32>,
    atlas: vec4<u32>,
    // Camera basis for screen-aligned expansion (billboards tilt with
    // pitch like the original's 2D screen blit).
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var t_sprites: texture_2d<u32>;
@group(0) @binding(2) var t_colormap: texture_2d<f32>;

struct Instance {
    // Feet-center world position (wrap-adjusted near the camera).
    @location(0) pos: vec3<f32>,
    // World-space quad size.
    @location(1) size: vec2<f32>,
    // Frame rect in atlas texels.
    @location(2) uv_pos: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    // x = horizontal mirror flag, y = shade LUT row.
    @location(4) flags: vec2<u32>,
    // Opacity: 1.0 opaque; 1/3 (smoke) / 2/3 (glows) for the retail
    // translucency raster modes. Only takes effect on the blend
    // pipeline (the opaque pipeline has blending disabled).
    @location(5) alpha: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) frac: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) uv_pos: vec2<f32>,
    @location(3) @interpolate(flat) uv_size: vec2<f32>,
    @location(4) @interpolate(flat) flags: vec2<u32>,
    // The sprite's painter-order depth, written for every fragment.
    // The depth channel carries HORIZONTAL camera distance (see
    // terrain.wgsl): the sprite is keyed to its anchor TILE's plan
    // distance minus half a tile — the original's "blit the sprite
    // right after its own tile's triangles" (sub_main.cpp :33673).
    // Walls the sprite stands against are farther tiles → never clip
    // it; tiles in front always hide it; ridge silhouettes still
    // occlude partially because the terrain side varies per pixel.
    @location(5) @interpolate(flat) anchor_depth: f32,
    @location(6) @interpolate(flat) alpha: f32,
};

const DEPTH_RANGE: f32 = 768.0;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    // Two triangles: corner x in {-0.5, 0.5}, y in {0 = feet, 1 = top}.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, 0.0), vec2<f32>(0.5, 0.0), vec2<f32>(0.5, 1.0),
        vec2<f32>(-0.5, 0.0), vec2<f32>(0.5, 1.0), vec2<f32>(-0.5, 1.0),
    );
    let c = corners[vid];
    let world = inst.pos
        + globals.cam_right.xyz * (c.x * inst.size.x)
        + globals.cam_up.xyz * (c.y * inst.size.y);
    var out: VsOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.frac = vec2<f32>(c.x + 0.5, 1.0 - c.y);
    out.world = world;
    out.uv_pos = inst.uv_pos;
    out.uv_size = inst.uv_size;
    out.flags = inst.flags;
    out.alpha = inst.alpha;
    let tile_center = floor(inst.pos.xz) + vec2<f32>(0.5, 0.5);
    out.anchor_depth = clamp(
        (length(tile_center - globals.camera.xz) - 0.5) / DEPTH_RANGE,
        0.0,
        0.999999,
    );
    return out;
}

struct FsOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(in: VsOut) -> FsOut {
    var fx = in.frac.x;
    if in.flags.x != 0u {
        fx = 1.0 - fx;
    }
    let texel = vec2<i32>(
        i32(in.uv_pos.x) + min(i32(fx * in.uv_size.x), i32(in.uv_size.x) - 1),
        i32(in.uv_pos.y) + min(i32(in.frac.y * in.uv_size.y), i32(in.uv_size.y) - 1),
    );
    let index = textureLoad(t_sprites, texel, 0).r;
    // Palette index 0 = transparent (the original skips zero pixels).
    if index == 0u {
        discard;
    }
    let shade = i32(min(in.flags.y, 63u));
    let base = textureLoad(t_colormap, vec2<i32>(i32(index), shade), 0).rgb;

    let dist = distance(in.world, globals.camera.xyz);
    let fog = 1.0 - exp(-dist * globals.camera.w);
    let rgb = mix(base, globals.fog_color.rgb, fog);
    var out: FsOut;
    out.color = vec4<f32>(rgb, in.alpha);
    out.depth = in.anchor_depth;
    return out;
}
