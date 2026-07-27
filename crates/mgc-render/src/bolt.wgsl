// PROTOTYPE lightning-bolt pass (enhanced lightning) — thin glowing
// RIBBONS, one instance per bolt segment. Same premultiplied blend
// regime as fire.wgsl (One / OneMinusSrcAlpha): the bolt is almost
// purely additive (a white-hot core with a pale blue-violet sheath),
// with only a whisper of occlusion so it still reads over bright sky.
//
// Each instance carries the segment's two world endpoints; the vertex
// stage builds a camera-facing ribbon quad by expanding perpendicular
// to both the segment axis and the view ray. The app feeds the whole
// bolt (main channel + branches) as consecutive segments per frame,
// with the strike envelope (leader → return stroke → decay) already
// baked into `energy`/`alpha`.

struct Globals {
    view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    fog_color: vec4<f32>,
    atlas: vec4<u32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct Instance {
    // World-space segment endpoints (wrap-adjusted near the camera).
    @location(0) p0: vec3<f32>,
    @location(1) p1: vec3<f32>,
    // World half-width of the ribbon.
    @location(2) width: f32,
    // 0..1 strike energy (return stroke = 1, leader/decay < 1).
    @location(3) energy: f32,
    // Overall coverage multiplier (envelope fade).
    @location(4) alpha: f32,
    // Per-strike procedural phase (flicker seed, time-rolled).
    @location(5) seed: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Cross-axis coordinate in [-1, 1] (0 = channel core).
    @location(0) across: f32,
    // Along-segment coordinate in WIDTH units, spanning
    // [-1, len/width + 1] (the ±1 is the capsule cap extension).
    @location(1) along: f32,
    // Segment length in WIDTH units.
    @location(2) @interpolate(flat) seg_w: f32,
    @location(3) @interpolate(flat) energy: f32,
    @location(4) @interpolate(flat) alpha: f32,
    @location(5) @interpolate(flat) seed: f32,
    @location(6) @interpolate(flat) depth: f32,
    @location(7) fog: f32,
};

const DEPTH_RANGE: f32 = 768.0;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: Instance) -> VsOut {
    // Two triangles: (-1,0)(1,0)(1,1) + (-1,0)(1,1)(-1,1); x = across,
    // y = endpoint lerp.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vid];
    var a = inst.p0;
    var b = inst.p1;
    // Water-reflection MIRROR pass (atlas.w = 2): hang the bolt
    // upside-down under the sea plane (see fire.wgsl / billboard.wgsl).
    if globals.atlas.w == 2u {
        a.y = -a.y;
        b.y = -b.y;
    }
    let axis = b - a;
    let seg_len = max(length(axis), 0.0001);
    let axis_n = axis / seg_len;
    // CAPSULE EXTENSION: stretch the quad one width past each
    // endpoint and let the fragment stage round the caps — adjacent
    // segments then CROSS-FADE at their (kinked) joints instead of
    // butting hard quad edges together, which read as XOR-style
    // notches once the ribbon got wide.
    let ext = inst.width;
    let a2 = a - axis_n * ext;
    let b2 = b + axis_n * ext;
    let mid = mix(a2, b2, c.y);
    let view = mid - globals.camera.xyz;
    var side = cross(axis, view);
    let sl = length(side);
    if sl > 0.0001 {
        side = side / sl;
    } else {
        // Segment aimed dead at the camera: any perpendicular works.
        side = globals.cam_right.xyz;
    }
    let world = mid + side * (c.x * inst.width);
    var out: VsOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.across = c.x;
    let span = seg_len / inst.width;
    out.along = mix(-1.0, span + 1.0, c.y);
    out.seg_w = span;
    out.energy = inst.energy;
    out.alpha = inst.alpha;
    out.seed = inst.seed;
    // Plan-distance depth, biased ~1 tile toward the camera so the
    // glow wins against the victim's opaque sprite at the terminus
    // (same law as fire.wgsl).
    out.depth = clamp(
        (length(mid.xz - globals.camera.xz) - 1.0) / DEPTH_RANGE,
        0.0,
        0.999999,
    );
    // Distance fog (retail band law), per corner.
    out.fog = 0.0;
    let d = globals.camera.w;
    if d > 0.0 {
        let dv = world - globals.camera.xyz;
        let dist2 = dot(dv, dv);
        let start2 = 0.5625 * d * d;
        let end2 = 0.9025 * d * d;
        out.fog = clamp((dist2 - start2) / (end2 - start2), 0.0, 1.0);
    }
    return out;
}

struct FsOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(in: VsOut) -> FsOut {
    // Capsule signed distance in width units: inside the segment span
    // the profile is the plain cross-axis distance; past an endpoint
    // it rounds off radially (the cap), so overlapping neighbors
    // cross-fade smoothly through their shared joint.
    let over = max(max(-in.along, in.along - in.seg_w), 0.0);
    let x = min(sqrt(in.across * in.across + over * over), 1.0);
    // White-hot core: a hard, thin center line.
    let core = smoothstep(0.42, 0.04, x);
    // Pale blue-violet sheath falling off to the ribbon edge.
    let sheath = (1.0 - x) * (1.0 - x);
    // High-frequency intensity flicker rides the time-rolled seed.
    let flick = 0.86 + 0.14 * sin(in.seed * 27.0 + in.along * 2.0);
    let vis = (1.0 - in.fog) * in.alpha * flick;
    let e = clamp(in.energy, 0.0, 1.0);
    let core_i = core * e * vis;
    let sheath_i = sheath * e * e * 0.45 * vis;
    if core_i + sheath_i < 0.01 {
        discard;
    }
    let white = vec3<f32>(1.0, 1.0, 1.0);
    let sheath_col = vec3<f32>(0.45, 0.55, 1.0);
    // OVER-compositing (premultiplied): the bolt paints its own COLOR
    // at its coverage, neither additive nor occluding. Overlapping
    // layers CONVERGE to the bolt color — a second strike or a branch
    // crossing the channel makes it a touch more solid, never a
    // white-out blob (additive) and never a darkened seam (the
    // occlusion XOR). Coverage caps just under 1 so a whisper of the
    // scene bleeds through even the core.
    let w = core_i + sheath_i;
    let a = clamp(w, 0.0, 0.92);
    let col = (white * core_i + sheath_col * sheath_i) / max(w, 0.004);
    var out: FsOut;
    out.color = vec4<f32>(col * a, a);
    out.depth = in.depth;
    return out;
}
