// Parallax cloud-plane sky: the 256x256 8bpp sky bitmap (baked
// `sky.bin`, CPU-resolved through the variant palette), tiled
// infinitely on both axes and steered by the camera direction.
//
// Retail (remc2 DrawSky_40950, GRO:258-370) texture-maps the bitmap
// ~1:1 across the 320px viewport with a 16-bit wrapping index, scrolls
// U by yaw at 4 full texture wraps per 360° revolution, slides V with
// pitch, and rotates the per-column stepping basis by the roll angle.
// We reproduce the same LAW from the true 3D view ray instead of the
// screen-space delta walk: per pixel, reconstruct the ray, take its
// azimuth/elevation, and map both at retail's 1024 texels per turn —
// yaw scroll, pitch slide and roll all fall out of the basis. At the
// retail 90°-ish FOV that is the same ~256 texels across the screen.
//
// Drawn as one oversized triangle before the terrain (depth write off,
// compare Always) so the world paints over it; the flat fog-color
// clear/fill stays underneath as the sky-off fallback.

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    fog_color: vec4<f32>,
    atlas: vec4<u32>,
    // xyz = the rolled camera basis; w slots carry tan(fov/2):
    // cam_right.w = horizontal, cam_up.w = vertical.
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var t_sky: texture_2d<f32>;
@group(0) @binding(2) var s_sky: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
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
    out.ndc = p[vi];
    return out;
}

const TAU: f32 = 6.283185307179586;
// Retail scroll law: a full 360° yaw sweep scrolls 4 texture wraps =
// 1024 texels (GRO:318 — `yaw << 15` on the 2048-unit circle).
const TEXELS_PER_TURN: f32 = 1024.0;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let right = globals.cam_right.xyz;
    let up = globals.cam_up.xyz;
    let fwd = cross(up, right);
    var dir = normalize(
        fwd + right * (in.ndc.x * globals.cam_right.w) + up * (in.ndc.y * globals.cam_up.w),
    );
    // The water-reflection MIRROR pass (atlas.w = 2): the sky at
    // infinity reflects about the sea plane by negating the ray's
    // vertical component — clouds appear in the water.
    if globals.atlas.w == 2u {
        dir.y = -dir.y;
    }
    // Azimuth in the yaw convention (0 = -Z, matching camera_basis).
    let az = atan2(dir.x, -dir.z);
    let el = asin(clamp(dir.y, -1.0, 1.0));
    let scale = TEXELS_PER_TURN / TAU / 256.0; // texture wraps per radian
    let u = az * scale;
    // The horizon samples the texture's BOTTOM EDGE (the wrap seam,
    // v = 1.0) — retail's exact anchor, decoded from DrawSky's fixed-
    // point walk (remc2 GameRenderHD.cpp:449-451 ≡ remc1 :38322-25;
    // MC1 is the same routine family): at pitch 0 `addY = Height/2`,
    // `beginY = -cosRoll*addY` with cosRoll = 256/Width texels per
    // pixel puts screen-center (the horizon) at row 256 ≡ 0 and the
    // screen top at row ~176. So the WHOLE authored bitmap lives
    // above the horizon (the MC2 night sky's moonlit rims at rows
    // 140-200 sit ~20-40 degrees up), and below the horizon only the
    // texture's dark top rows wrap into view — the earlier 0.60
    // empirical anchor hung the cloud band below eye level (player
    // report 2026-07-16 "sky too low, black above"). The in-frame
    // texel density stays at the U scale: retail's own frame shows
    // ~0.8 texel/line ≈ this ray law (its 2x-rate pitch SLIDE is a
    // screen-space scroll artifact a world-anchored sky can't and
    // shouldn't copy).
    let v = 1.0 - el * scale;
    return vec4<f32>(textureSample(t_sky, s_sky, vec2<f32>(u, v)).rgb, 1.0);
}
