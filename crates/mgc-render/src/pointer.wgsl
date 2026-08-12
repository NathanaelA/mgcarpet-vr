// VR controller pointer beam: a short world-space line from the
// controller to the UI panel hit. Drawn as a camera-facing ribbon so it
// has a consistent thickness in stereo.

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    fog_color: vec4<f32>,
    atlas: vec4<u32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    billboard_right: vec4<f32>,
    billboard_up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct Instance {
    // World-space segment endpoints.
    @location(0) p0: vec3<f32>,
    @location(1) p1: vec3<f32>,
    // World half-width of the ribbon.
    @location(2) width: f32,
    // RGBA tint (linear, premultiplied-friendly).
    @location(3) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Cross-axis coordinate in [-1, 1], 0 = beam center.
    @location(0) across: f32,
    // Along-segment coordinate in width units, [-1, len/width + 1].
    @location(1) along: f32,
    // Segment length in width units.
    @location(2) @interpolate(flat) seg_w: f32,
    @location(3) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vid];
    let a = inst.p0;
    let b = inst.p1;
    let axis = b - a;
    let seg_len = max(length(axis), 0.0001);
    let axis_n = axis / seg_len;

    // Extend one width past each endpoint so the fragment stage can round
    // the caps, matching the bolt pass's capsule look.
    let a2 = a - axis_n * inst.width;
    let b2 = b + axis_n * inst.width;
    let mid = mix(a2, b2, c.y);
    let view = mid - globals.camera.xyz;
    var side = cross(axis, view);
    let sl = length(side);
    if sl > 0.0001 {
        side = side / sl;
    } else {
        side = globals.billboard_right.xyz;
    }
    let world = mid + side * (c.x * inst.width);

    var out: VsOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.across = c.x;
    let span = seg_len / inst.width;
    out.along = mix(-1.0, span + 1.0, c.y);
    out.seg_w = span;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let over = max(max(-in.along, in.along - in.seg_w), 0.0);
    let x = min(sqrt(in.across * in.across + over * over), 1.0);
    // Hard inner core + soft outer edge.
    let alpha = (1.0 - x) * in.color.a;
    if alpha < 0.01 {
        discard;
    }
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
