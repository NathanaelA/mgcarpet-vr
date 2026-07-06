// Screen-space UI sprites (spellbook icons, HUD slots, mana bars).
// Quads live in pixel coordinates, origin top-left; the atlas is
// RGBA, pre-composited on the CPU through the engine's blend LUT
// (`blend[src | dest<<8]`, the original's 2D blit path) so the shader
// stays a dumb textured blit. A zero-width UV rect means "solid quad"
// (mana-bar fills, dim overlays) drawn from the tint alone.

struct UiGlobals {
    // Surface size in pixels (z/w unused).
    screen: vec4<f32>,
};

@group(0) @binding(0) var<uniform> ui: UiGlobals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct Instance {
    // x, y, w, h in pixels.
    @location(0) rect: vec4<f32>,
    // u, v, w, h in texels; w == 0 -> solid tint quad.
    @location(1) uv: vec4<f32>,
    @location(2) tint: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) solid: u32,
    @location(2) @interpolate(flat) tint: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[vid];
    let px = inst.rect.xy + c * inst.rect.zw;
    var out: VsOut;
    // Pixel -> NDC (y down in pixels, up in NDC).
    out.clip = vec4<f32>(
        px.x / ui.screen.x * 2.0 - 1.0,
        1.0 - px.y / ui.screen.y * 2.0,
        0.0,
        1.0,
    );
    let tex_size = vec2<f32>(textureDimensions(atlas_tex));
    out.uv = (inst.uv.xy + c * inst.uv.zw) / tex_size;
    out.solid = u32(inst.uv.z == 0.0);
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if in.solid == 1u {
        return in.tint;
    }
    // Nearest-texel chunky sampling, like every sprite path here.
    let c = textureSample(atlas_tex, atlas_samp, in.uv);
    if c.a < 0.5 {
        discard;
    }
    return c * in.tint;
}
