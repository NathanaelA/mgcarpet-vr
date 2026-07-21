// Supersample resolve: draw the offscreen scene buffer to the surface
// with one oversized triangle and a LINEAR sampler.
//
// The scene is rendered at `render_scale`x the window and averaged back
// down here, which is plain supersampling: it antialiases everything the
// frame contains — terrain and sprite silhouettes in the 3D view, and
// equally the 2D UI, whose scaled-up sprite edges are the other thing
// that shows stair-stepping. Cheaper AA (MSAA) would only smooth
// geometry edges and would leave the UI exactly as it is.
//
// The downscale is a bilinear tap rather than a full box filter: at 2x
// that samples 4 texels weighted evenly, which IS the box, and at
// fractional scales it is a close approximation.

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

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
    let xy = p[vi];
    out.clip = vec4<f32>(xy, 0.0, 1.0);
    // Clip space -> uv (y flips: NDC is up, texture rows are down).
    out.uv = vec2<f32>(xy.x * 0.5 + 0.5, 0.5 - xy.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(scene_tex, scene_samp, in.uv);
}
