// Separable gaussian over the water-reflection mirror image (player
// ask 2026-08-08: "reflections have always been kind of too clean" —
// retail's 320x200 reflection block was inherently soft, a modern-res
// pixel-perfect mirror is not). Two fullscreen passes: fs_h samples
// the FULL-res mirror into a DIV-times-smaller target, fs_v blurs
// that vertically into the texture the water fragments sample; the
// bilinear upsample at the water blend adds its own free softening.
//
// 5-tap kernel with linear-sampling offsets (a 9-texel footprint):
// the classic 0.227/0.316/0.070 weights at offsets 0 / 1.3846 /
// 3.2308 texels.

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.clip = vec4<f32>(p[vi], 0.0, 1.0);
    out.uv = vec2<f32>(p[vi].x, -p[vi].y) * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

// Downsample factor of the blur targets — MUST match lib.rs
// REFLECTION_BLUR_DIV (the H pass reads the full-res mirror, so its
// tap spacing scales up to the destination's texel pitch).
const DIV: f32 = 2.0;

const W0: f32 = 0.2270270270;
const W1: f32 = 0.3162162162;
const W2: f32 = 0.0702702703;
const O1: f32 = 1.3846153846;
const O2: f32 = 3.2307692308;

fn blur(uv: vec2<f32>, step: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(t_src, s_src, uv, 0.0) * W0
        + textureSampleLevel(t_src, s_src, uv + step * O1, 0.0) * W1
        + textureSampleLevel(t_src, s_src, uv - step * O1, 0.0) * W1
        + textureSampleLevel(t_src, s_src, uv + step * O2, 0.0) * W2
        + textureSampleLevel(t_src, s_src, uv - step * O2, 0.0) * W2;
}

@fragment
fn fs_h(in: VsOut) -> @location(0) vec4<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(t_src));
    return blur(in.uv, vec2<f32>(px.x * DIV, 0.0));
}

@fragment
fn fs_v(in: VsOut) -> @location(0) vec4<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(t_src));
    return blur(in.uv, vec2<f32>(0.0, px.y));
}
