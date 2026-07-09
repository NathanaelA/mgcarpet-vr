// Viewport-filling solid sky: one oversized triangle clipped to the
// current viewport/scissor rect. Used behind the world viewport on the
// book screen, where the pass itself clears to the dark book backdrop.
// Color = the globals' fog_color (linear) — the environment sky the
// renderer uploads per frame (SKY_SRGB default / set_sky_color).

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    fog_color: vec4<f32>,
    atlas: vec4<u32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

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
    return vec4<f32>(globals.fog_color.rgb, 1.0);
}
