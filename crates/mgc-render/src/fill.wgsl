// Viewport-filling solid sky: one oversized triangle clipped to the
// current viewport/scissor rect. Used behind the world viewport on the
// book screen, where the pass itself clears to the dark book backdrop.
// Color = linear form of the renderer's SKY_SRGB constant (lib.rs) —
// keep in sync.

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.1474, 0.2635, 0.5226, 1.0);
}
